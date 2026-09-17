#[derive(Debug)]
#[allow(unused)]
enum AuthMethod {
    NoAuth,
    // 其他认证方法可根据需求扩展
}

#[allow(unused)]
impl AuthMethod {
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
#[allow(unused)]
enum Command {
    Connect,
    // 可根据需求支持Bind和UDP Associate
}

#[allow(unused)]
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
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub(crate) async fn handle_client(
    mut client: TcpStream,
    route_engine: Arc<RouteEngine>,
) -> Result<()> {
    // 1. 认证协商: VER NMETHODS METHODS...
    let mut head = [0u8; 2];
    client.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        return Err(anyhow!("Unsupported SOCKS version"));
    }
    let mut methods = vec![0u8; head[1] as usize];
    client.read_exact(&mut methods).await?;
    client.write_all(&[0x05, 0x00]).await?; // 无认证

    // 2. 请求: VER CMD RSV ATYP DST.ADDR DST.PORT
    let mut head = [0u8; 4];
    client.read_exact(&mut head).await?;
    if head[0] != 0x05 || head[1] != 0x01 {
        return Err(anyhow!("Unsupported SOCKS request"));
    }
    let address = match head[3] {
        0x01 => {
            // IPv4 + port
            let mut a = [0u8; 6];
            client.read_exact(&mut a).await?;
            format!(
                "{}.{}.{}.{}:{}",
                a[0],
                a[1],
                a[2],
                a[3],
                u16::from_be_bytes([a[4], a[5]])
            )
        }
        0x03 => {
            // 域名 + port
            let mut len = [0u8; 1];
            client.read_exact(&mut len).await?;
            let mut d = vec![0u8; len[0] as usize + 2];
            client.read_exact(&mut d).await?;
            let port = u16::from_be_bytes([d[d.len() - 2], d[d.len() - 1]]);
            format!("{}:{}", String::from_utf8_lossy(&d[..d.len() - 2]), port)
        }
        t => return Err(anyhow!("Unsupported address type {t}")),
    };

    // 3. 先连目标，成功后才回 RFC1928 成功响应
    let server = match TcpStream::connect(&address).await {
        Ok(s) => {
            client
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            s
        }
        Err(e) => {
            let _ = client
                .write_all(&[0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            return Err(e.into());
        }
    };

    let rule = match crate::core::http::parse_http_header(&client).await {
        Some((host, _path)) => route_engine.resolve_by_host(&host),
        None => None,
    };

    let Some(rule) = rule else {
        // 未命中路由：双向透传
        let (mut cr, mut cw) = tokio::io::split(client);
        let (mut sr, mut sw) = tokio::io::split(server);
        tokio::try_join!(
            tokio::io::copy(&mut cr, &mut sw),
            tokio::io::copy(&mut sr, &mut cw)
        )?;
        return Ok(());
    };
    crate::core::http::forward_handle(client, server, &rule).await
}

// #[tokio::test]
#[allow(unused)]
async fn test_socks() -> Result<()> {
    use tokio::net::TcpListener;
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
            let route_engine = Arc::new(RouteEngine {
                rules: std::sync::RwLock::new(vec![rule]),
            });
            if let Err(e) = handle_client(socket, route_engine).await {
                error!("Error handling client: {}", e);
            }
        });
    }
}
