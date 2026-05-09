use anyhow::{Context, Result};
use clap::Parser;
use std::thread;
use tokio::net::TcpListener;
use tracing::{debug, info};

use net4mqtt::broker;
use tracing::{debug, info};

mod log;
mod utils;

use tokio::net::TcpListener;

use net4mqtt::broker;

const COBOTSRV_NAME: &str = "cobotsrv";
const COBOTRIX_NAME: &str = "cobotrix";
const MQTT_BROKER_ADDR: &str = "0.0.0.0:1883";

#[derive(Parser)]
#[command(name = "cobotnil", version)]
struct Args {
    /// Set config file path
    #[arg(short, long, default_value_t = format!("cobotnil"))]
    config: String,
}

pub fn parse(name: &str) -> (&str, &str, &str) {
    let mut v: Vec<&str> = name.split('.').collect();
    if v.len() < 2 {
        v[1] = ""
    };
    (v[0], v[1], v[v.len() - 1])
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load cobotsrv config
    let cobotsrv_cfg: cobotsrv::config::Config = utils::load(
        COBOTSRV_NAME.to_string(),
        args.config
            .clone()
            .map(|s| format!("{s}/{COBOTSRV_NAME}.toml")),
    );

    // Load cobotrix config
    let cobotrix_cfg: cobotrix::config::Config = utils::load(
        COBOTRIX_NAME.to_string(),
        args.config
            .clone()
            .map(|s| format!("{s}/{COBOTRIX_NAME}.toml")),
    );

    log::set(format!(
        "cobotrix={},cobotnil={},cobotsrv={},cobotlet={},http_log={}",
        cobotsrv_cfg.log.level,
        cobotsrv_cfg.log.level,
        cobotsrv_cfg.log.level,
        cobotsrv_cfg.log.level,
        cobotsrv_cfg.log.level
    ));

    debug!("config : {:?}", cfg);

    let broker_addr = MQTT_BROKER_ADDR.parse()?;
    // Start MQTT broker
    let broker_addr = MQTT_BROKER_ADDR
        .parse()
        .context("invalid MQTT broker address")?;
    thread::spawn(move || broker::up_mqtt_broker(broker_addr));
    broker::wait_for_port_availabilty(broker_addr).await;

    // Start cobotlet instances
    let mut dir_entries = tokio::fs::read_dir(&args.config)
        .await
        .context("failed to read config directory")?;
    let mut cobotlet_nodes = Vec::new();
    while let Some(entry) = dir_entries
        .next_entry()
        .await
        .context("failed to read directory entry")?
    {
        let file_name = entry.file_name().to_str().unwrap().to_string();
        let (srv, alias, _) = parse(&file_name);
        // Skip cobotsrv and cobotrix configs
        if srv == COBOTSRV_NAME || srv == COBOTRIX_NAME {
            continue;
        }
        let alias = alias.to_string();

        let cfg: cobotlet::config::Config = utils::load(
            "cobotlet".to_string(),
            entry.path().to_str().map(|s| s.to_string()),
        );
        debug!("config : {:?}", cfg);
        let listener = TcpListener::bind(&cfg.http.listen)
            .await
            .context("failed to bind cobotlet listener")?;
        let addr = listener.local_addr()?;
        info!("cobotlet[{alias}] listening on {}", addr);
        cobotlet_nodes.push(cobotrix::config::Node {
            alias: alias.to_string(),
            url: format!("http://{addr}"),
            ..Default::default()
        });

        tokio::spawn(cobotlet::serve(cfg, listener, utils::shutdown_signal()));
    }

    // Start cobotsrv (auth service)
    let cobotsrv_listener = TcpListener::bind(&cobotsrv_cfg.http.listen)
        .await
        .context("failed to bind cobotsrv listener")?;
    info!("cobotsrv listening on {}", cobotsrv_listener.local_addr()?);
    tokio::spawn(cobotsrv::serve(
        cobotsrv_cfg,
        cobotsrv_listener,
        utils::shutdown_signal(),
    ));

    // Start cobotrix (management center) — registers cobotlet nodes
    let mut cobotrix_cfg = cobotrix_cfg;
    cobotrix_cfg.nodes.extend(cobotlet_nodes);
    let cobotrix_listener = TcpListener::bind(&cobotrix_cfg.http.listen)
        .await
        .context("failed to bind cobotrix listener")?;
    info!("cobotrix listening on {}", cobotrix_listener.local_addr()?);
    cobotrix::serve(cobotrix_cfg, cobotrix_listener, utils::shutdown_signal()).await;

    info!("Server shutdown");
    Ok(())
}
