use clap::Parser;
use tracing::{debug, info, warn};

mod log;
mod utils;

#[derive(Parser)]
#[command(version)]
struct Args {
    /// Set config file path
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() {
    //cobotlet::metrics_register();
    let args = Args::parse();
    let cfg: cobotlet::config::Config = utils::load("cobotlet".to_string(), args.config);
    //cfg.validate().unwrap();
    log::set(format!(
        "cobotrix={},cobotlet={},net4mqtt={},http_log={}",
        cfg.log.level, cfg.log.level, cfg.log.level, cfg.log.level
    ));
    warn!("set log level : {}", cfg.log.level);
    debug!("config : {:?}", cfg);
    let listener = tokio::net::TcpListener::bind(&cfg.http.listen)
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    info!("Server listening on {}", addr);

    cobotlet::serve(cfg, listener, utils::shutdown_signal()).await;
    info!("Server shutdown");
}
