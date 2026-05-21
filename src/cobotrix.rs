use std::path::Path;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use tokio::net::TcpListener;
use tracing::{Level, debug, info, warn};

mod log;
mod utils;

const COBOTUSR_NAME: &str = "cobotusr";
const COBOTSRV_NAME: &str = "cobotsrv";

#[derive(Parser)]
#[command(name = "cobotrix", version)]
struct Args {
    /// Verbose mode [default: "warn", -v "info", -vv "debug", -vvv "trace"]
    #[arg(short = 'v', action = ArgAction::Count)]
    verbose: u8,
    /// Config directory containing cobotusr.toml and cobotsrv.toml
    #[arg(short, long, default_value_t = String::from("conf/cobotrix"))]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load cobotusr config (auth / IdP / PDP)
    let cobotusr_config_path = format!("{}/{COBOTUSR_NAME}.toml", args.config);
    let path = Path::new(&cobotusr_config_path);
    let cobotusr_cfg: cobotusr::config::Config = if path.try_exists()? {
        toml::from_str(std::fs::read_to_string(path)?.as_str())?
    } else {
        eprintln!("=== No cobotusr config file, use default config ===");
        Default::default()
    };

    // Load cobotsrv config (device management server)
    let cobotsrv_config_path = format!("{}/{COBOTSRV_NAME}.toml", args.config);
    let path = Path::new(&cobotsrv_config_path);
    let cobotsrv_cfg: cobotsrv::config::Config = if path.try_exists()? {
        toml::from_str(std::fs::read_to_string(path)?.as_str())?
    } else {
        eprintln!("=== No cobotsrv config file, use default config ===");
        Default::default()
    };

    let log_level = if args.verbose != 0 {
        match args.verbose {
            1 => Level::INFO,
            2 => Level::DEBUG,
            _ => Level::TRACE,
        }
        .to_string()
    } else {
        cobotusr_cfg.log.level.to_ascii_uppercase()
    };

    log::set(format!(
        "cobotsrv={level},cobotusr={level},net4mqtt={level},http_log={level}",
        level = log_level,
    ));

    warn!("set log level: [{}]", log_level);
    debug!("cobotusr config : {:?}", cobotusr_cfg);
    debug!("cobotsrv config : {:?}", cobotsrv_cfg);

    // Start cobotusr (IdP + PDP) as background task
    let cobotusr_listener = TcpListener::bind(&cobotusr_cfg.http.listen)
        .await
        .context("failed to bind cobotusr listener")?;
    info!("cobotusr listening on {}", cobotusr_listener.local_addr()?);
    tokio::spawn(cobotusr::serve(
        cobotusr_cfg,
        cobotusr_listener,
        utils::shutdown_signal(),
    ));

    // Start cobotsrv (device management) — awaited, holds the process
    let cobotsrv_listener = TcpListener::bind(&cobotsrv_cfg.http.listen)
        .await
        .context("failed to bind cobotsrv listener")?;
    info!("cobotsrv listening on {}", cobotsrv_listener.local_addr()?);
    cobotsrv::serve(cobotsrv_cfg, cobotsrv_listener, utils::shutdown_signal()).await;

    info!("Server graceful shutdown completed");
    Ok(())
}
