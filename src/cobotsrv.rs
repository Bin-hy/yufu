use clap::Parser;
use tracing::{debug, info, warn};

mod log;
mod utils;

#[derive(Parser)]
#[command(name = "cobotsrv", version)]
struct Args {
    /// Set config file path
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let cfg: cobotsrv::config::Config = utils::load("cobotsrv".to_string(), args.config);

    #[cfg(debug_assertions)]
    log::set(format!(
        "cobotrix={},cobotsrv={},net4mqtt={},http_log={}",
        cfg.log.level, cfg.log.level, cfg.log.level, cfg.log.level
    ));

    #[cfg(not(debug_assertions))]
    log::set(format!(
        "cobotrix={},cobotsrv={},http_log={}",
        cfg.log.level, cfg.log.level, cfg.log.level
    ));

    warn!("set log level : {}", cfg.log.level);
    debug!("config : {:?}", cfg);

    let listener = tokio::net::TcpListener::bind(cfg.http.listen)
        .await
        .unwrap();

    cobotsrv::serve(cfg, listener, utils::shutdown_signal()).await;
    info!("Server shutdown");
}
