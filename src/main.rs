use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{error, info};

mod core;
mod libs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    libs::logs::init_default()?;

    let listener = TcpListener::bind("127.0.0.1:1080").await?;

    info!("SOCKS5 proxy listening on 127.0.0.1:1080");

    let rule = core::route::RouteRule::new("192.168.120.177:81", "/api", "127.0.0.1:8686", "");
    let mut vec = Vec::new();
    vec.push(rule);
    let rules = Arc::new(RwLock::new(vec));
    let route_engine = Arc::new(core::route::RouteEngine { rules });

    loop {
        let (socket, _) = listener.accept().await?;
        let engine = route_engine.clone();
        tokio::spawn(async move {
            if let Err(e) = core::socks::handle_client(socket, engine).await {
                error!("Error handling client: {}", e);
            }
        });
    }
}
