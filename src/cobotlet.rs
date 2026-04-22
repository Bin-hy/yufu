use std::path::Path;

use anyhow::Result;
use clap::{ArgAction, Parser};
use tracing::{Level, debug, info, warn};

mod log;
mod utils;

#[derive(Parser)]
#[command(version)]
struct Args {
    /// Verbose mode [default: "warn", -v "info", -vv "debug", -vvv "trace"]
    #[arg(short = 'v', action = ArgAction::Count)]
    verbose: u8,
    /// Set config file path
    #[arg(short, long, default_value_t = format!("{}.toml", "cobotlet"))]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let path = Path::new(&args.config);

    let cfg: cobotlet::config::Config = if path.try_exists()? {
        toml::from_str(std::fs::read_to_string(path)?.as_str())?
    } else {
        eprintln!("=== No any config file, use default config ===");
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
        cfg.log.level.to_ascii_uppercase()
    };

    log::set(format!(
        "cobotrix={},cobotlet={},net4mqtt={},http_log={}",
        log_level, log_level, log_level, log_level
    ));
    warn!("set log level: [{}]", log_level);
    debug!("config : {:?}", cfg);
    let listener = tokio::net::TcpListener::bind(&cfg.http.listen).await?;
    let addr = listener.local_addr()?;
    info!("Server listening on {}", addr);

    cobotlet::serve(cfg, listener, utils::shutdown_signal()).await;
    info!("Server graceful shutdown completed");

    Ok(())
}
