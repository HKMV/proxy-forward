use crate::core::config::AppConfig;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

mod core;
mod libs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    libs::logs::init_default()?;
    let config = AppConfig::init().expect("读取配置文件失败");

    let listener = TcpListener::bind(config.listen_addr.as_str()).await?;
    info!("SOCKS5 proxy listening on {}", config.listen_addr);

    let rules = config
        .rules
        .iter()
        .map(|r| {
            core::route::RouteRule::new(
                &r.matcher.addr,
                &r.matcher.path_prefix,
                &r.forward.addr,
                &r.forward.path_prefix,
            )
        })
        .collect();
    let route_engine = Arc::new(core::route::RouteEngine {
        rules: std::sync::RwLock::new(rules),
    });
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
