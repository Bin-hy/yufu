use anyhow::{Context, Result};
use clap::Parser;
use std::path::Path;
use std::thread;
use tokio::net::TcpListener;
use tracing::{debug, info};

use net4mqtt::broker;

mod log;
mod utils;

const COBOTSRV_NAME: &str = "cobotsrv";
const COBOTRIX_NAME: &str = "cobotrix";
const MQTT_BROKER_ADDR: &str = "0.0.0.0:1883";

#[derive(Parser)]
#[command(name = "cobotnil", version)]
struct Args {
    /// Set config directory path (e.g. "conf/cobotnil")
    #[arg(short, long, default_value_t = String::from("conf/cobotnil"))]
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

fn load_cfg<T: serde::de::DeserializeOwned + Default>(path: &Path, label: &str) -> T {
    if path.try_exists().unwrap_or(false) {
        toml::from_str(
            std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read {label}: {e}"))
                .as_str(),
        )
        .unwrap_or_else(|e| panic!("failed to parse {label}: {e}"))
    } else {
        eprintln!("=== No {label} config file, using defaults ===");
        T::default()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load cobotsrv config
    let cobotsrv_cfg: cobotsrv::config::Config = load_cfg(
        Path::new(&format!("{}/{COBOTSRV_NAME}.toml", args.config)),
        COBOTSRV_NAME,
    );

    // Load cobotrix config
    let cobotrix_cfg: cobotrix::config::Config = load_cfg(
        Path::new(&format!("{}/{COBOTRIX_NAME}.toml", args.config)),
        COBOTRIX_NAME,
    );

    log::set(format!(
        "cobotrix={level},cobotnil={level},cobotsrv={level},cobotlet={level},http_log={level}",
        level = cobotsrv_cfg.log.level,
    ));

    debug!("cobotsrv config : {:?}", cobotsrv_cfg);
    debug!("cobotrix config : {:?}", cobotrix_cfg);

    // Start embedded MQTT broker (net4mqtt)
    let broker_addr = MQTT_BROKER_ADDR.parse()?;
    thread::spawn(move || broker::up_mqtt_broker(broker_addr));
    broker::wait_for_port_availabilty(broker_addr).await;

    // Scan config directory for cobotlet configs (files not named cobotsrv/cobotrix)
    let mut cobotlet_nodes: Vec<cobotrix::config::Node> = Vec::new();
    let mut dir_entries = tokio::fs::read_dir(&args.config).await?;
    while let Some(entry) = dir_entries.next_entry().await? {
        let file_name = entry.file_name().to_str().unwrap_or("").to_string();
        let (srv, alias, ext) = parse(&file_name);
        if ext != "toml" || srv == COBOTSRV_NAME || srv == COBOTRIX_NAME {
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

        cobotlet_nodes.push(cobotrix::config::Node {
            alias: alias.clone(),
            url: format!("http://{addr}"),
            ..Default::default()
        });

        tokio::spawn(cobotlet::serve(cfg, listener, utils::shutdown_signal()));
    }

    // Start cobotsrv (IdP + PDP)
    let cobotsrv_listener = TcpListener::bind(&cobotsrv_cfg.http.listen)
        .await
        .context("failed to bind cobotsrv listener")?;
    info!("cobotsrv listening on {}", cobotsrv_listener.local_addr()?);
    tokio::spawn(cobotsrv::serve(
        cobotsrv_cfg,
        cobotsrv_listener,
        utils::shutdown_signal(),
    ));

    // Start cobotrix (device management center) — inject discovered cobotlet URLs
    let mut cobotrix_cfg = cobotrix_cfg;
    cobotrix_cfg.nodes.extend(cobotlet_nodes);
    let cobotrix_listener = TcpListener::bind(&cobotrix_cfg.http.listen)
        .await
        .context("failed to bind cobotrix listener")?;
    info!("cobotrix listening on {}", cobotrix_listener.local_addr()?);
    cobotrix::serve(cobotrix_cfg, cobotrix_listener, utils::shutdown_signal()).await;

    info!("Server graceful shutdown completed");
    Ok(())
}
