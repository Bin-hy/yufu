use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread;

use portpicker::pick_unused_port;
use tokio::net::TcpListener;
use tracing::debug;

use net4mqtt::broker;

const MQTT_TOPIC_PREFIX: &str = "net4mqtt";

use serde::{Deserialize, Serialize};
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Hello {
    #[serde(default)]
    pub alias: String,
}

pub async fn shutdown_signal() {
    let str = signal::wait_for_stop_signal().await;
    debug!("Received signal: {}", str);
}

#[cfg(feature = "net4mqtt")]
#[tokio::test]
#[ignore]
async fn test_http_cluster() {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mqtt_broker_port: u16 = pick_unused_port().expect("No ports free");
    let mqtt_broker_host = ip;
    let broker_addr = SocketAddr::new(mqtt_broker_host, mqtt_broker_port);
    thread::spawn(move || broker::up_mqtt_broker(broker_addr));
    broker::wait_for_port_availabilty(broker_addr).await;

    let devices: [_; 3] = std::array::from_fn(|i| {
        use cobotlet::config::{Config, Http, Net4mqtt};
        Config {
            http: Http {
                listen: SocketAddr::new(ip, 0),
                ..Default::default()
            },
            net4mqtt: Some(Net4mqtt {
                alias: format!("test-cobotlet-{}", i),
                mqtt_url: format!(
                    "mqtt://{}/{}?client_id=test-cobotlet-{}",
                    broker_addr, MQTT_TOPIC_PREFIX, i
                ),
            }),
            ..Default::default()
        }
    });

    let rs = devices.clone().map(async |cfg| {
        let listener = TcpListener::bind(&cfg.http.listen).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let alias = cfg.clone().net4mqtt.unwrap().alias;
        tokio::spawn(cobotlet::serve(cfg, listener, shutdown_signal()));
        (alias, addr)
    });

    let results = futures::future::join_all(rs).await;

    // cobotrix (management center) — handles remote proxy to cobotlets
    let mut cobotrix_cfg = {
        use cobotrix::config::{Config, Http};
        Config {
            http: Http {
                listen: SocketAddr::new(ip, 0),
                ..Default::default()
            },
            ..Default::default()
        }
    };
    cobotrix_cfg.nodes.extend(
        results
            .into_iter()
            .map(|(alias, addr)| cobotrix::config::Node {
                alias: alias.to_string(),
                url: format!("http://{addr}"),
                ..Default::default()
            }),
    );

    let listener = TcpListener::bind(cobotrix_cfg.http.listen).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(cobotrix::serve(cobotrix_cfg, listener, shutdown_signal()));

    let rs = devices.map(async |cfg| {
        let id = cfg.net4mqtt.unwrap().alias;
        let client = reqwest::Client::new();
        let url = format!("http://{}/remote/{}/hello", addr, id);

        let response = client
            .get(&url)
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);

        let hello: Hello = response.json().await.expect("Failed to parse JSON");

        assert_eq!(hello.alias, id);
    });
    futures::future::join_all(rs).await;
}
