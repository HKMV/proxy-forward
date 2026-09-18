mod core;
mod daemon;
mod libs;
mod ui;

fn main() -> anyhow::Result<()> {
    if std::env::args().any(|a| a == "--headless") {
        libs::logs::init_default()?;
        let config = core::config::AppConfig::init()?;
        let stats = core::stats::Stats::shared();
        tokio::runtime::Runtime::new()?.block_on(async move {
            let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::select! {
                r = daemon::run(config, rx, stats) => r?,
                _ = tokio::signal::ctrl_c() => {}
            }
            Ok(())
        })
    } else {
        ui::run()
    }
}
