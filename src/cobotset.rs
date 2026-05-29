use std::path::Path;

use anyhow::Result;
use clap::{ArgAction, Parser};
use tracing::{Level, debug, warn};

mod log;

#[derive(Parser)]
#[command(version, about = "Register a device with the local cobotlet")]
struct Args {
    /// Verbose mode [default: "warn", -v "info", -vv "debug", -vvv "trace"]
    #[arg(short = 'v', action = ArgAction::Count)]
    verbose: u8,
    /// Config file path
    #[arg(short, long, default_value_t = format!("{}.toml", "cobotset"))]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let path = Path::new(&args.config);

    let cfg: cobotset::config::Config = if path.try_exists()? {
        toml::from_str(&std::fs::read_to_string(path)?)?
    } else {
        eprintln!("Config file not found: {}", args.config);
        eprintln!();
        eprintln!("Create a cobotset.toml — example:");
        eprintln!();
        eprintln!("{}", cobotset::EXAMPLE_CONFIG);
        anyhow::bail!("config file not found");
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

    log::set(format!("cobotrix={log_level},cobotset={log_level}"));
    warn!("log level: [{log_level}]");
    debug!("config: {:?}", cfg);

    cobotset::run(cfg).await
}
