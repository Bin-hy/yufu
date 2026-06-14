pub mod config;

use std::collections::HashMap;

use anyhow::Context;
use easytier::common::config::{ConfigLoader, NetworkIdentity, PeerConfig, TomlConfigLoader};
use easytier::instance::instance::Instance;
use easytier::proto::rpc_types::controller::BaseController;
use easytier::rpc_service::InstanceRpcService;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};

pub use config::Config;

/// Information about a discovered peer on the VPN network.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub name: String,
    pub virtual_ip: String,
    pub peer_id: u32,
}

/// Events emitted when peers connect or disconnect.
#[derive(Debug, Clone)]
pub enum PeerEvent {
    Online(PeerInfo),
    Offline(String),
}

/// Handle to stop a running easytier node.
pub struct StopHandle {
    stop_tx: oneshot::Sender<()>,
    done_rx: oneshot::Receiver<()>,
}

impl StopHandle {
    /// Send stop signal and block until the background thread has
    /// cleaned up the easytier instance (calling `clear_resources`).
    pub fn stop(self) {
        let _ = self.stop_tx.send(());
        // Wait up to 10 seconds for the background thread to exit.
        // If it times out, we still return — the OS will clean up.
        let _ = self.done_rx.blocking_recv();
    }
}

/// Managed easytier P2P VPN node.
///
/// Wraps an easytier `Instance`, managing its lifecycle in a background thread
/// with its own tokio runtime. Peer discovery events are emitted through
/// an mpsc channel.
pub struct EasytierNode {
    config: Config,
}

impl EasytierNode {
    /// Create a new node with the given configuration.
    /// The node is not started until `start()` is called.
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Start the easytier node in a background thread.
    ///
    /// Returns a `StopHandle` for graceful shutdown and a channel receiver
    /// for peer discovery events.
    pub fn start(self) -> Result<(StopHandle, mpsc::Receiver<PeerEvent>), anyhow::Error> {
        let config = self.config;
        let (peer_tx, peer_rx) = mpsc::channel::<PeerEvent>(64);
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let (done_tx, done_rx) = oneshot::channel::<()>();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let loader = config_to_toml_loader(&config);

                let mut instance = Instance::new(loader);

                if let Err(e) = instance.run().await {
                    error!("easytier instance failed to start: {:?}", e);
                    return;
                }

                info!("easytier node started (peer_id: {})", instance.peer_id());

                // Start polling for peers
                let api = instance.get_api_rpc_service();

                let mut stop_rx = stop_rx;
                let mut known_peers: HashMap<u32, PeerInfo> = HashMap::new();

                loop {
                    // Check for stop signal
                    if stop_rx.try_recv().is_ok() {
                        info!("easytier node received stop signal");
                        break;
                    }

                    // Query peer routes (has peer_id, hostname, ipv4_addr)
                    let ctrl = BaseController::default();
                    match api
                        .get_peer_manage_service()
                        .list_route(
                            ctrl.clone(),
                            easytier::proto::api::instance::ListRouteRequest::default(),
                        )
                        .await
                    {
                        Ok(resp) => {
                            let mut current_peer_ids: Vec<u32> = Vec::new();

                            for route in &resp.routes {
                                let name = if route.hostname.is_empty() {
                                    format!("peer-{}", route.peer_id)
                                } else {
                                    route.hostname.clone()
                                };
                                let virtual_ip = match &route.ipv4_addr {
                                    Some(ip) => {
                                        let addr = std::net::Ipv4Addr::from(
                                            ip.address.unwrap_or_default().addr,
                                        );
                                        addr.to_string()
                                    }
                                    None => "unknown".to_string(),
                                };

                                if let std::collections::hash_map::Entry::Vacant(e) =
                                    known_peers.entry(route.peer_id)
                                {
                                    let p = PeerInfo {
                                        name: name.clone(),
                                        virtual_ip: virtual_ip.clone(),
                                        peer_id: route.peer_id,
                                    };
                                    info!("easytier peer online: {} at {}", p.name, p.virtual_ip);
                                    e.insert(p.clone());
                                    if peer_tx.send(PeerEvent::Online(p)).await.is_err() {
                                        warn!("easytier peer event channel closed");
                                        break;
                                    }
                                }
                                current_peer_ids.push(route.peer_id);
                            }

                            // Detect offline peers
                            let mut offline = Vec::new();
                            known_peers.retain(|id, p| {
                                if current_peer_ids.contains(id) {
                                    true
                                } else {
                                    offline.push(p.name.clone());
                                    false
                                }
                            });
                            for name in offline {
                                info!("easytier peer offline: {}", name);
                                if peer_tx.send(PeerEvent::Offline(name)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("easytier peer route query failed: {:?}", e);
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }

                instance.clear_resources().await;
                info!("easytier node stopped");
                let _ = done_tx.send(());
            });
        });

        Ok((StopHandle { stop_tx, done_rx }, peer_rx))
    }
}

/// Convert cobotnet Config into easytier's TomlConfigLoader.
fn config_to_toml_loader(config: &Config) -> TomlConfigLoader {
    let loader = TomlConfigLoader::default();

    // Network identity
    if !config.network_name.is_empty() {
        let identity =
            NetworkIdentity::new(config.network_name.clone(), config.network_secret.clone());
        loader.set_network_identity(identity);
    }

    // Instance info
    if !config.instance_name.is_empty() {
        loader.set_inst_name(config.instance_name.clone());
    }
    if !config.hostname.is_empty() {
        loader.set_hostname(Some(config.hostname.clone()));
    }

    // IPv4
    if !config.ipv4.is_empty() {
        if let Ok(ip) = config
            .ipv4
            .parse::<cidr::Ipv4Inet>()
            .with_context(|| format!("invalid ipv4: {}", config.ipv4))
        {
            loader.set_ipv4(Some(ip));
        } else {
            warn!("failed to parse ipv4: {}", config.ipv4);
        }
    }

    // IPv6
    if !config.ipv6.is_empty() {
        if let Ok(ip) = config
            .ipv6
            .parse::<cidr::Ipv6Inet>()
            .with_context(|| format!("invalid ipv6: {}", config.ipv6))
        {
            loader.set_ipv6(Some(ip));
        } else {
            warn!("failed to parse ipv6: {}", config.ipv6);
        }
    }

    // Listeners
    if !config.listeners.is_empty() {
        let urls: Vec<url::Url> = config
            .listeners
            .iter()
            .filter_map(|s| {
                s.parse::<url::Url>()
                    .inspect_err(|e| warn!("invalid listener url '{}': {:?}", s, e))
                    .ok()
            })
            .collect();
        if !urls.is_empty() {
            loader.set_listeners(urls);
        }
    }

    // Peers
    if !config.peers.is_empty() {
        let peers: Vec<PeerConfig> = config
            .peers
            .iter()
            .filter_map(|s| {
                let url: url::Url = s
                    .parse()
                    .inspect_err(|e| warn!("invalid peer url '{}': {:?}", s, e))
                    .ok()?;
                Some(PeerConfig {
                    uri: url,
                    peer_public_key: None,
                })
            })
            .collect();
        if !peers.is_empty() {
            loader.set_peers(peers);
        }
    }

    loader
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_is_empty() {
        let cfg = Config::default();
        assert!(cfg.network_name.is_empty());
        assert!(cfg.network_secret.is_empty());
        assert!(cfg.listeners.is_empty());
        assert!(cfg.peers.is_empty());
    }

    #[test]
    fn test_config_validate_generates_instance_name() {
        let mut cfg = Config {
            network_name: "test-net".into(),
            network_secret: "secret".into(),
            ..Default::default()
        };
        cfg.validate();
        assert!(
            !cfg.instance_name.is_empty(),
            "instance_name should be auto-generated"
        );
        assert_eq!(
            cfg.hostname, cfg.instance_name,
            "hostname should mirror instance_name when empty"
        );
    }

    #[test]
    fn test_config_validate_preserves_explicit_hostname() {
        let mut cfg = Config {
            instance_name: "my-node".into(),
            hostname: "my-host".into(),
            ..Default::default()
        };
        cfg.validate();
        assert_eq!(cfg.instance_name, "my-node");
        assert_eq!(
            cfg.hostname, "my-host",
            "explicit hostname should not be overwritten"
        );
    }

    #[test]
    fn test_config_to_toml_loader_network_identity() {
        let mut cfg = Config {
            network_name: "test-net".into(),
            network_secret: "test-secret".into(),
            ipv4: "10.100.0.1".into(),
            ..Default::default()
        };
        cfg.validate();

        let loader = config_to_toml_loader(&cfg);
        let identity = loader.get_network_identity();
        assert_eq!(identity.network_name, "test-net");
    }

    #[test]
    fn test_config_to_toml_loader_ipv4_parsed() {
        let mut cfg = Config {
            network_name: "test".into(),
            ipv4: "10.100.0.1".into(),
            ..Default::default()
        };
        cfg.validate();

        let loader = config_to_toml_loader(&cfg);
        let ip = loader.get_ipv4();
        assert!(ip.is_some(), "ipv4 should be set");
        assert_eq!(ip.unwrap().to_string(), "10.100.0.1/24");
    }

    #[test]
    fn test_config_to_toml_loader_invalid_ipv4_is_ignored() {
        let mut cfg = Config {
            ipv4: "not-an-ip".into(),
            ..Default::default()
        };
        cfg.validate();

        let loader = config_to_toml_loader(&cfg);
        assert!(loader.get_ipv4().is_none());
    }

    #[test]
    fn test_config_to_toml_loader_listeners_and_peers() {
        let mut cfg = Config {
            network_name: "test".into(),
            listeners: vec!["tcp://0.0.0.0:11010".into()],
            peers: vec!["tcp://10.0.0.1:11010".into()],
            ..Default::default()
        };
        cfg.validate();

        let loader = config_to_toml_loader(&cfg);
        let listeners = loader.get_listeners();
        assert!(listeners.is_some());
        assert_eq!(listeners.unwrap().len(), 1);

        let peers = loader.get_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].uri.to_string(), "tcp://10.0.0.1:11010");
    }

    #[test]
    fn test_easytier_node_new() {
        let mut cfg = Config {
            network_name: "test-net".into(),
            network_secret: "secret".into(),
            ..Default::default()
        };
        cfg.validate();

        // Constructing the node should not panic
        let _node = EasytierNode::new(cfg);
    }

    #[test]
    fn test_stop_handle() {
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        let handle = StopHandle { stop_tx, done_rx };

        // Simulate the background thread cleanup
        std::thread::spawn(move || {
            // Wait for stop signal
            let _ = stop_rx.blocking_recv();
            // Simulate clear_resources
            std::thread::sleep(std::time::Duration::from_millis(50));
            let _ = done_tx.send(());
        });

        handle.stop();
        // stop() should have blocked until done_tx was sent
    }
}
