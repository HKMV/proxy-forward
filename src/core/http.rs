use crate::core::route::RouteRule;
use anyhow::Result;
use httparse::Status;
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, error};

pub(crate) async fn forward_handle(
    client: TcpStream,
    server: TcpStream,
    rule: &RouteRule,
) -> Result<()> {
    // 创建缓冲区用于协议检测
    let mut peek_buf = [0u8; 256];
    let peek_size = client.peek(&mut peek_buf).await?;
    let is_http = is_http(peek_buf.as_ref(), peek_size);

    // 拆分客户端和服务器流为读写两半
    let (mut client_reader, mut client_writer) = tokio::io::split(client);
    let (mut server_reader, mut server_writer) = tokio::io::split(server);

    let forward = match TcpStream::connect(&rule.forward.host).await {
        Ok(ts) => Some(ts),
        Err(e) => {
            if rule.forward.connect_fail_use_original_host {
                error!("Connect to forward host failed, use original host: {}", e);
                let client_to_target = io::copy(&mut client_reader, &mut server_writer);
                let target_to_client = io::copy(&mut server_reader, &mut client_writer);
                tokio::try_join!(client_to_target, target_to_client)?;
                return Ok(());
            }

            if let Some(path) = parse_path(peek_buf.as_ref())
                && path.starts_with(rule.match_.prefix.as_str())
            {
                //转发服务连接不上，终止需要转发的请求
                error!("Connect to forward host failed, stop access: {}", e);

                if let Err(e) = client_writer
                    .write_all(service_unavailable().as_slice())
                    .await
                {
                    error!("Forward write error: {}", e);
                }

                server_writer.shutdown().await.unwrap_or(());
                client_writer.shutdown().await.unwrap_or(());
                return Err(e.into());
            }

            // 不进行修改转发的路径继续访问
            None
        }
    };
    let (mut forward_reader, mut forward_writer) = match forward {
        None => (None, None),
        Some(f) => {
            let (reader, writer) = io::split(f);
            (Some(reader), Some(writer))
        }
    };

    let client_writer = Arc::new(Mutex::new(client_writer));
    let rule = rule.clone();

    let client_writer_c = client_writer.clone();
    let client_to_server = tokio::spawn(async move {
        let mut buf = [0u8; 8192];
        let mut flag = 0;
        loop {
            let n = match client_reader.read(&mut buf).await {
                Ok(n) if n == 0 => break, // EOF
                Ok(n) => n,
                Err(e) => {
                    error!("Client read error: {}", e);
                    break;
                }
            };
            debug!("flag: {}", flag);
            flag += 1;

            // 如果是HTTP流量，可以进行修改
            if !is_http {
                if let Err(e) = server_writer.write_all(&buf[..n]).await {
                    error!("Server write error: {}", e);
                    break;
                }
                continue;
            }
            match modify_http_data(&mut buf[..n], &rule) {
                None => {
                    if let Err(e) = server_writer.write_all(&buf[..n]).await {
                        error!("Server write error: {}", e);
                        break;
                    }
                }
                Some(d) => {
                    if let Some(fw) = &mut forward_writer {
                        debug!("forward_writer: {}", &rule.forward.host);
                        if let Err(e) = fw.write_all(&*d).await {
                            error!("Forward write error: {}", e);
                            break;
                        }
                    } else if let Err(e) = client_writer_c
                        .lock()
                        .await
                        .write_all(service_unavailable().as_slice())
                        .await
                    {
                        error!("Forward write error: {}", e);
                        break;
                    }
                }
            }
        }
        server_writer.shutdown().await.unwrap_or(());
        if let Some(fw) = &mut forward_writer {
            fw.shutdown().await.unwrap_or(());
        }
    });

    let server_to_client = tokio::spawn(async move {
        let client_writer0 = client_writer.clone();
        let handle0 = tokio::spawn(async move {
            let mut buf = [0u8; 8192];
            loop {
                let n = match server_reader.read(&mut buf).await {
                    Ok(n) if n == 0 => break, // EOF
                    Ok(n) => n,
                    Err(e) => {
                        error!("Server read error: {}", e);
                        break;
                    }
                };

                if let Err(e) = client_writer0.lock().await.write_all(&buf[..n]).await {
                    error!("Client write error: {}", e);
                    break;
                }
            }
        });

        let client_writer1 = client_writer.clone();
        let handle1 = tokio::spawn(async move {
            if let Some(fr) = &mut forward_reader {
                let mut buf = [0u8; 8192];
                loop {
                    let n = match fr.read(&mut buf).await {
                        Ok(n) if n == 0 => break, // EOF
                        Ok(n) => n,
                        Err(e) => {
                            error!("Forward read error: {}", e);
                            break;
                        }
                    };
                    if let Err(e) = client_writer1.lock().await.write_all(&buf[..n]).await {
                        error!("Client write error: {}", e);
                        break;
                    }
                }
            }
        });

        let _ = tokio::try_join!(handle0, handle1);
        client_writer.lock().await.shutdown().await.unwrap_or(());
    });

    // 等待两个方向的任务完成
    let _ = tokio::try_join!(client_to_server, server_to_client);
    debug!("Request handle finished");
    Ok(())
}

fn modify_http_data(data: &mut [u8], rule: &RouteRule) -> Option<Vec<u8>> {
    if data.len() >= 4 && data.starts_with(b"HTTP/") {
        //响应数据
        return None;
    }

    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);

    match req.parse(data) {
        Ok(Status::Complete(_pos)) => {
            // 解析成功，修改URL
            if let Some(path) = req.path {
                // debug!("Original data: {}", String::from_utf8_lossy(data));
                // debug!("Original data: {:?}", std::str::from_utf8(data));

                // 创建新的URL路径
                let prefix = &rule.match_.prefix;
                if prefix.is_empty()
                    || prefix.eq("/")
                    || !path.starts_with(prefix)
                    || !rule.forward.rewrite
                {
                    return None;
                };
                debug!("Original URL path: {}", path);
                let new_path = path.replacen(prefix, &rule.forward.prefix, 1);
                debug!("Modified URL path: {}", new_path);

                let data_str = std::str::from_utf8(data)
                    .unwrap()
                    .to_string()
                    // .replacen("\r\nConnection: keep-alive", "", 1)
                    .replacen(path, new_path.as_str(), 1);
                Some(data_str.into_bytes())
            } else {
                None
            }
        }
        _ => {
            // 非完整HTTP请求或不支持的格式
            None
        }
    }
}

fn is_http(data: &[u8], size: usize) -> bool {
    size >= 4
        && (data.starts_with(b"GET ")
            || data.starts_with(b"POST")
            || data.starts_with(b"PUT ")
            || data.starts_with(b"PATCH ")
            || data.starts_with(b"DELETE ")
            || data.starts_with(b"HTTP/"))
}

fn parse_path(data: &[u8]) -> Option<&str> {
    // 找到请求行的结束位置\r\n
    let end = data.windows(2).position(|w| w == [b'\r', b'\n'])?;
    let request_line = &data[..end];

    // 按空格分割并过滤空字段
    let mut parts = request_line
        .split(|&b| b == b' ')
        .filter(|part| !part.is_empty());

    // 跳过方法（第一个字段），取路径（第二个字段）
    parts.next()?; // 方法
    let path_bytes = parts.next()?; // 路径
    parts.next()?; // 协议版本（可选检查，确保存在）

    std::str::from_utf8(path_bytes).ok()
}

pub(crate) async fn parse_http_header(stream: &TcpStream) -> Option<(String, String)> {
    let mut buf = [0u8; 4096];
    let n = stream.peek(&mut buf).await.ok()?;
    if !is_http(buf.as_ref(), n) {
        //非HTTP请求
        return None;
    }

    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);
    // debug!("req_str {}", String::from_utf8_lossy(&buf[..n]));

    let _status = req.parse(&buf[..n]).ok()?;
    let path = req.path?.to_string();
    let host = req
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("host"))
        .and_then(|h| std::str::from_utf8(h.value).ok())?
        .to_string();
    // debug!("req_path {path}");
    Some((host, path))
}

fn service_unavailable() -> Vec<u8> {
    let response = b"HTTP/1.1 503 Service Unavailable\r\n\
        Content-Type: text/plain\r\n\
        Content-Length: 17\r\n\
        Connection: close\r\n\r\n\
        Service Unavailable";
    response.to_vec()
}
