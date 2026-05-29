use std::path::Path;

use anyhow::Result;
use clap::Parser;
use tracing::{debug, info};

mod log;
#[allow(dead_code)]
mod utils;

#[derive(Parser)]
#[command(name = "cobotmap", version)]
struct Args {
    /// Set config file path
    #[arg(short, long, default_value_t = format!("{}.toml", "cobotmap"))]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let path = Path::new(&args.config);

    let cfg: cobotmap::Config = if path.try_exists()? {
        toml::from_str(std::fs::read_to_string(path)?.as_str())?
    } else {
        eprintln!("=== No any config file, use default config ===");
        Default::default()
    };

    log::set(format!("cobotmap={}", cfg.log.level));
    info!("set log level: {}", cfg.log.level);
    debug!("config : {:?}", cfg);

    if let Err(err) = cobotmap::run(cfg).await {
        tracing::error!(error = %err, "cobotmap exited with error");
        std::process::exit(1);
    }

    info!("cobotmap shutdown");
    Ok(())
}
