use std::path::Path;

use anyhow::Result;
use clap::{ArgAction, Parser};
use tracing::{Level, debug, info, warn};

mod log;
mod utils;

#[derive(Parser)]
#[command(name = "yufu", version)]
struct Args {
    /// Verbose mode [default: "warn", -v "info", -vv "debug", -vvv "trace"]
    #[arg(short = 'v', action = ArgAction::Count)]
    verbose: u8,
    /// Set config file path
    #[arg(short, long, default_value_t = format!("{}.toml", "yufu"))]
    config: String,
    /// Allow HTTP cookies for local browser development; requires loopback listener/issuer.
    #[arg(long)]
    allow_insecure_loopback_cookies: bool,
    /// Check configured NTP clock health and exit without opening HTTP or the database.
    #[arg(long)]
    check_security_clock: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let path = Path::new(&args.config);

    let mut cfg: yufu::config::Config = if path.try_exists()? {
        toml::from_str(std::fs::read_to_string(path)?.as_str())?
    } else {
        eprintln!("=== No any config file, use default config ===");
        Default::default()
    };
    if args.allow_insecure_loopback_cookies {
        cfg.auth.allow_insecure_loopback_cookies = true;
    }
    cfg.validate_browser_cookie_policy()?;

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

    log::set(format!("yufu={},http_log={}", log_level, log_level));

    warn!("set log level: [{}]", log_level);
    if cfg.auth.allow_insecure_loopback_cookies {
        warn!("explicit loopback HTTP development cookies enabled");
    } else if cfg.http.listen.ip().is_loopback()
        && cfg.http.public.is_empty()
        && cfg.oidc.issuer.is_empty()
        && !args.check_security_clock
    {
        warn!(
            "local HTTP is using Secure cookies; browser development may require --allow-insecure-loopback-cookies"
        );
    }
    debug!(
        listen = %cfg.http.listen,
        api_prefix = %cfg.http.api_prefix,
        log_level = %cfg.log.level,
        "config loaded"
    );

    let monitor = yufu::clock::ClockMonitor::start(&cfg.security_clock).await?;
    if args.check_security_clock {
        let status = monitor.status()?;
        println!(
            "security_clock: synchronized={}, max_error_millis={:?}, sample_age_millis={:?}",
            status.synchronized, status.max_error_millis, status.sample_age_millis
        );
        return Ok(());
    }
    cfg.validate_identity_policy()?;
    cfg.load_identity_signer().await?;
    let listener = tokio::net::TcpListener::bind(cfg.http.listen).await?;
    let addr = listener.local_addr()?;
    info!("Server listening on {}", addr);

    yufu::serve_with_security_clock(cfg, listener, utils::shutdown_signal(), monitor.clock()).await;
    info!("Server graceful shutdown completed");

    Ok(())
}
