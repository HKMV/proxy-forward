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
use crate::core::stats::Stats;
use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub(crate) async fn handle_client(
    mut client: TcpStream,
    route_engine: Arc<RouteEngine>,
    stats: Arc<Stats>,
) -> Result<()> {
    let _guard = stats.conn_open();
    // 协议探测：首字节 0x05 = SOCKS5，否则按 HTTP 代理处理
    let mut probe = [0u8; 1];
    client.peek(&mut probe).await?;
    if probe[0] != 0x05 {
        return crate::core::http::handle_http_proxy(client, route_engine, stats).await;
    }
    // 1. 认证协商: VER NMETHODS METHODS...
    let mut head = [0u8; 2];
    client.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        return Err(anyhow!("Unsupported SOCKS version"));
    }
    let mut methods = vec![0u8; head[1] as usize];
    client.read_exact(&mut methods).await?;

    // 选择无认证方法
    client.write_all(&[0x05, AuthMethod::NoAuth.to_u8()]).await?;

    // 2. 处理请求: VER CMD RSV ATYP ADDR PORT
    client.read_exact(&mut head).await?;
    let mut atyp = [0u8; 2]; // RSV + ATYP
    client.read_exact(&mut atyp).await?;
    if head[0] != 0x05 {
        return Err(anyhow!("Unsupported SOCKS version in request"));
    }
    let _cmd = Command::from_u8(head[1]).ok_or(anyhow!("Unsupported command"))?;

    let address = match atyp[1] {
        0x01 => {
            // IPv4
            let mut addr = [0u8; 6];
            client.read_exact(&mut addr).await?;
            format!(
                "{}.{}.{}.{}:{}",
                addr[0],
                addr[1],
                addr[2],
                addr[3],
                u16::from_be_bytes([addr[4], addr[5]])
            )
        }
        0x03 => {
            // Domain name
            let mut len = [0u8; 1];
            client.read_exact(&mut len).await?;
            let mut rest = vec![0u8; len[0] as usize + 2];
            client.read_exact(&mut rest).await?;
            let domain = String::from_utf8_lossy(&rest[..len[0] as usize]);
            let port = u16::from_be_bytes([rest[rest.len() - 2], rest[rest.len() - 1]]);
            format!("{}:{}", domain, port)
        }
        _ => return Err(anyhow!("Unsupported address type")),
    };

    // 3. 发送成功响应
    client
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    // 4. 连接目标服务器
    client.set_nodelay(true).ok();
    let server = TcpStream::connect(&address)
        .await
        .with_context(|| format!("connect {address}"))?;
    server.set_nodelay(true).ok();

    let route_rule = match crate::core::http::parse_http_header(&client).await {
        // 不是http请求或解析失败
        None => None,
        Some((host, _path)) => route_engine.resolve_target_by_host(&host).await,
    };

    let Some(rule) = route_rule else {
        // 未匹配到路由规则,直接转发
        let (mut client_reader, mut client_writer) = tokio::io::split(client);
        let (mut server_reader, mut server_writer) = tokio::io::split(server);
        // ponytail: 字节数在 copy 结束后一次性累计，非实时；要实时曲线再换带计数的 copy
        let (up, down) = tokio::try_join!(
            tokio::io::copy(&mut client_reader, &mut server_writer),
            tokio::io::copy(&mut server_reader, &mut client_writer)
        )?;
        stats.up(up as usize);
        stats.down(down as usize);
        return Ok(());
    };
    crate::core::http::forward_handle(client, server, &rule, &stats).await
}

// #[tokio::test]
#[allow(unused)]
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
            if let Err(e) = handle_client(socket, route_engine, crate::core::stats::Stats::shared()).await {
                error!("Error handling client: {}", e);
            }
        });
    }
}
