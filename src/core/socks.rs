#[derive(Debug)]
enum AuthMethod {
    NoAuth,
    // 其他认证方法可根据需求扩展
}

impl AuthMethod {
    #[allow(unused)]
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(AuthMethod::NoAuth),
            _ => None,
        }
    }
    fn to_u8(&self) -> u8 {
        match self {
            AuthMethod::NoAuth => 0x00,
        }
    }
}

#[derive(Debug)]
enum Command {
    Connect,
    // 可根据需求支持Bind和UDP Associate
}

impl Command {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Command::Connect),
            _ => None,
        }
    }
}
use crate::core::route::RouteEngine;
use anyhow::{Result, anyhow};
use bytes::{BufMut, BytesMut};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub(crate) async fn handle_client(
    mut client: TcpStream,
    route_engine: Arc<RouteEngine>,
) -> Result<()> {
    // 1. 认证协商
    let mut buf = BytesMut::with_capacity(256);
    client.read_buf(&mut buf).await?;

    if buf[0] != 0x05 {
        return Err(anyhow!("Unsupported SOCKS version"));
    }

    let methods_count = buf[1] as usize;
    let _methods = &buf[2..2 + methods_count];

    // 选择无认证方法
    let mut response = vec![0x05, AuthMethod::NoAuth.to_u8()]; // VER, METHOD
    client.write_all(&response).await?;
    buf.clear();

    // 2. 处理请求
    client.read_buf(&mut buf).await?;
    if buf[0] != 0x05 {
        return Err(anyhow!("Unsupported SOCKS version in request"));
    }
    let _cmd = Command::from_u8(buf[1]).ok_or(anyhow!("Unsupported command"))?;
    let address_type = buf[3];

    let address = match address_type {
        0x01 => {
            // IPv4
            let addr = &buf[4..8];
            format!(
                "{}.{}.{}.{}:{}",
                addr[0],
                addr[1],
                addr[2],
                addr[3],
                u16::from_be_bytes([buf[8], buf[9]])
            )
        }
        0x03 => {
            // Domain name
            let len = buf[4] as usize;
            let domain = String::from_utf8_lossy(&buf[5..5 + len]).to_string();
            let port = u16::from_be_bytes([buf[5 + len], buf[5 + len + 1]]);
            format!("{}:{}", domain, port)
        }
        _ => return Err(anyhow!("Unsupported address type")),
    };

    // 3. 发送成功响应
    response.clear();
    response.put_u8(0x05); // VER
    response.put_u8(0x00); // SUCCESS
    response.put_u8(0x00); // RSV
    response.put_u8(0x01); // IPv4
    response.put_slice(&[0, 0, 0, 0]); // IP
    response.put_u16(0); // Port
    client.write_all(&response).await?;

    // 4. 连接目标服务器
    let server = TcpStream::connect(address).await?;

    let route_rule = match crate::core::http::parse_http_header(&client).await {
        // 不是http请求或解析失败
        None => None,
        Some((host, _path)) => match route_engine.resolve_target_by_host(&host).await {
            // 未匹配到路由规则
            None => None,
            Some(r) => Some(r),
        },
    };

    if let None = route_rule {
        // 拆分客户端和服务器流为读写两半
        let (mut client_reader, mut client_writer) = tokio::io::split(client);
        let (mut server_reader, mut server_writer) = tokio::io::split(server);
        let client_to_target = tokio::io::copy(&mut client_reader, &mut server_writer);
        let target_to_client = tokio::io::copy(&mut server_reader, &mut client_writer);
        tokio::try_join!(client_to_target, target_to_client)?;
        return Ok(());
    }
    let rule = route_rule.unwrap();
    crate::core::http::forward_handle(client, server, &rule).await?;
    Ok(())

    /*
    // 5. 数据转发
    let (mut client_reader, mut client_writer) = client.split();
    let (mut target_reader, mut target_writer) = target.split();

    let client_to_target = tokio::io::copy(&mut client_reader, &mut target_writer);
    let target_to_client = tokio::io::copy(&mut target_reader, &mut client_writer);

    tokio::select! {
        res = client_to_target => {
            if let Err(e) = res {
                if e.kind() != io::ErrorKind::ConnectionReset
                || e.kind() != io::ErrorKind::ConnectionAborted {
                    return Err(e.into());
                }
                debug!("Client closed connection");
            }
        }
        res = target_to_client => {
            if let Err(e) = res {
                if e.kind() != std::io::ErrorKind::ConnectionReset
                || e.kind() != std::io::ErrorKind::ConnectionAborted {
                    return Err(e.into());
                }
                debug!("Target server closed connection");
            }
        }
    }
    // tokio::try_join!(client_to_target, target_to_client)?;*/
}

#[tokio::test]
async fn test_socks() -> Result<()> {
    use tokio::net::TcpListener;
    use tokio::sync::RwLock;
    use tracing::{error, info};

    let listener = TcpListener::bind("127.0.0.1:1080").await?;
    info!("SOCKS5 proxy listening on 127.0.0.1:1080");

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let rule = crate::core::route::RouteRule::new(
                "192.168.120.177:81",
                "/api",
                "127.0.0.1:8686",
                "",
            );
            let mut vec = Vec::new();
            vec.push(rule);
            let rules = Arc::new(RwLock::new(vec));
            let route_engine = Arc::new(RouteEngine { rules });
            if let Err(e) = handle_client(socket, route_engine).await {
                error!("Error handling client: {}", e);
            }
        });
    }
}
