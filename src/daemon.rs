use crate::core::config::{AppConfig, Rule};
use crate::core::route::{RouteEngine, RouteRule};
use crate::core::stats::Stats;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{RwLock, mpsc};
use tracing::{error, info};

pub enum Command {
    SetRules(Vec<Rule>),
    Shutdown,
}

/// 在后台线程起一个 tokio runtime 跑代理，返回命令通道。
/// 发送 Shutdown（或 drop 通道）后线程自行退出；再次启动就再调一次。
pub fn spawn(config: AppConfig, stats: Arc<Stats>) -> mpsc::UnboundedSender<Command> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async move {
            if let Err(e) = run(config, rx, stats).await {
                error!("daemon exited with error: {e:#}");
            }
        });
    });
    tx
}

fn to_route_rules(rules: &[Rule]) -> Vec<RouteRule> {
    rules
        .iter()
        .map(|r| {
            RouteRule::new(
                &r.matcher.addr,
                &r.matcher.path_prefix,
                &r.forward.addr,
                &r.forward.path_prefix,
            )
        })
        .collect()
}

pub async fn run(
    config: AppConfig,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    stats: Arc<Stats>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&config.listen_addr).await?;
    info!("SOCKS5 proxy listening on {}", config.listen_addr);
    let engine = Arc::new(RouteEngine {
        rules: Arc::new(RwLock::new(to_route_rules(&config.rules))),
    });

    loop {
        tokio::select! {
            accept = listener.accept() => {
                let (socket, _) = accept?;
                let engine = engine.clone();
                let stats = stats.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::core::socks::handle_client(socket, engine, stats).await
                    {
                        error!("Error handling client: {}", e);
                    }
                });
            }
            cmd = cmd_rx.recv() => match cmd {
                Some(Command::SetRules(rules)) => {
                    let n = rules.len();
                    engine.update_rules(to_route_rules(&rules)).await;
                    info!("rules reloaded ({n} rules)");
                }
                Some(Command::Shutdown) | None => break,
            }
        }
    }
    info!("SOCKS5 proxy stopped");
    Ok(())
}
