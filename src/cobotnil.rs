use std::path::Path;
use std::thread;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::net::TcpListener;
use tracing::{debug, info};

use net4mqtt::broker;

mod log;
mod utils;

const COBOTUSR_NAME: &str = "cobotusr";
const COBOTSRV_NAME: &str = "cobotsrv";
const MQTT_BROKER_ADDR: &str = "0.0.0.0:1883";

#[derive(Parser)]
#[command(name = "cobotnil", version)]
struct Args {
    /// Set config directory path (e.g. "conf/cobotnil")
    #[arg(short, long, default_value_t = String::from("cobotnil"))]
    config: String,
}

/// Split a config filename like "cobotlet.0.toml" into (service, alias, ext).
pub fn parse(name: &str) -> (&str, &str, &str) {
    let mut v: Vec<&str> = name.split('.').collect();
    if v.len() < 2 {
        v.push("")
    };
    (v[0], v[1], v[v.len() - 1])
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load cobotusr config (auth / IdP service)
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

    log::set(format!(
        "cobotsrv={level},cobotnil={level},cobotusr={level},cobotlet={level},http_log={level}",
        level = cobotusr_cfg.log.level,
    ));

    debug!("cobotusr config : {:?}", cobotusr_cfg);
    debug!("cobotsrv config : {:?}", cobotsrv_cfg);

    // Start embedded MQTT broker (net4mqtt)
    let broker_addr = MQTT_BROKER_ADDR.parse()?;
    thread::spawn(move || broker::up_mqtt_broker(broker_addr));
    broker::wait_for_port_availabilty(broker_addr).await;

    // Scan config directory for cobotlet configs (files not named cobotusr/cobotsrv)
    let mut cobotlet_nodes: Vec<cobotsrv::config::Node> = Vec::new();
    let mut dir_entries = tokio::fs::read_dir(&args.config).await?;
    while let Some(entry) = dir_entries.next_entry().await? {
        let file_name = entry.file_name().to_str().unwrap_or("").to_string();
        let (srv, alias, ext) = parse(&file_name);
        if ext != "toml" || srv == COBOTUSR_NAME || srv == COBOTSRV_NAME {
            continue;
        }
        let alias = alias.to_string();

        let cfg: cobotlet::config::Config =
            toml::from_str(std::fs::read_to_string(entry.path())?.as_str())?;

        debug!("cobotlet[{alias}] config : {:?}", cfg);
        let listener = TcpListener::bind(&cfg.http.listen)
            .await
            .context("failed to bind cobotlet listener")?;
        let addr = listener.local_addr()?;
        info!("cobotlet[{alias}] listening on {addr}");

        cobotlet_nodes.push(cobotsrv::config::Node {
            alias: alias.clone(),
            url: format!("http://{addr}"),
            ..Default::default()
        });

        tokio::spawn(cobotlet::serve(cfg, listener, utils::shutdown_signal()));
    }

    // Start cobotusr (IdP + PDP)
    let cobotusr_listener = TcpListener::bind(&cobotusr_cfg.http.listen)
        .await
        .context("failed to bind cobotusr listener")?;
    info!("cobotusr listening on {}", cobotusr_listener.local_addr()?);
    tokio::spawn(cobotusr::serve(
        cobotusr_cfg,
        cobotusr_listener,
        utils::shutdown_signal(),
    ));

    // Start cobotsrv (device management server) — inject discovered cobotlet URLs
    let mut cobotsrv_cfg = cobotsrv_cfg;
    cobotsrv_cfg.nodes.extend(cobotlet_nodes);
    let cobotsrv_listener = TcpListener::bind(&cobotsrv_cfg.http.listen)
        .await
        .context("failed to bind cobotsrv listener")?;
    info!("cobotsrv listening on {}", cobotsrv_listener.local_addr()?);
    cobotsrv::serve(cobotsrv_cfg, cobotsrv_listener, utils::shutdown_signal()).await;

    info!("Server graceful shutdown completed");
    Ok(())
}
