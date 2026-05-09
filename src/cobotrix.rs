use anyhow::{Context, Result};
use clap::Parser;
use tracing::{debug, info, warn};

mod log;
mod utils;

#[derive(Parser)]
#[command(name = "cobotrix", version)]
struct Args {
    /// Set config file path
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let cfg: cobotrix::config::Config = utils::load("cobotrix".to_string(), args.config);

    #[cfg(debug_assertions)]
    log::set(format!(
        "cobotrix={},net4mqtt={},http_log={}",
        cfg.log.level, cfg.log.level, cfg.log.level
    ));

    #[cfg(not(debug_assertions))]
    log::set(format!(
        "cobotrix={},http_log={}",
        cfg.log.level, cfg.log.level
    ));

    warn!("set log level : {}", cfg.log.level);
    debug!("config : {:?}", cfg);

    let listener = tokio::net::TcpListener::bind(cfg.http.listen)
        .await
        .context("failed to bind listener")?;

    cobotrix::serve(cfg, listener, utils::shutdown_signal()).await;
    info!("Server shutdown");
    Ok(())
}
