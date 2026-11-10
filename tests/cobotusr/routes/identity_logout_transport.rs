//! Back-channel logout delivery against a real TLS receiver.
//!
//! The unit tests in `logout_transport` cover address policy and status
//! mapping. These tests stand up an HTTPS endpoint on loopback, issue its
//! certificate from a throwaway CA, and let the transport reach it through the
//! same code path production uses.
use auth::security::{
    budget::{Clock, SystemClock},
    identity::SecretToken,
};
use cobotusr::config::Identity;
use cobotusr::service::identity::{
    logout_dispatcher::LogoutTransport,
    logout_transport::{HttpsLogoutTransport, LogoutResolver},
    repository_outbox::DeliveryOutcome,
};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair};
use rustls::{
    ServerConfig,
    crypto::ring,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};
use tokio_rustls::TlsAcceptor;

const HOST: &str = "rp.example";

#[derive(Clone)]
struct Reply {
    status: u16,
    retry_after: Option<&'static str>,
    delay: Duration,
}

impl Reply {
    fn status(status: u16) -> Self {
        Self {
            status,
            retry_after: None,
            delay: Duration::ZERO,
        }
    }
}

struct Receiver {
    address: SocketAddr,
    seen: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Issues a CA and a leaf certificate for [`HOST`], then serves TLS on loopback.
async fn receiver(reply: Reply) -> (Receiver, reqwest::Certificate) {
    let mut ca_params = CertificateParams::new(Vec::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "cobot logout test CA");
    let ca_key = KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();

    let mut leaf_params = CertificateParams::new(vec![HOST.to_owned()]).unwrap();
    leaf_params
        .distinguished_name
        .push(DnType::CommonName, HOST);
    let leaf_key = KeyPair::generate().unwrap();
    let leaf_cert = leaf_params.signed_by(&leaf_key, &ca_cert, &ca_key).unwrap();

    let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(leaf_cert.der().to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
        )
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = seen.clone();
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            let reply = reply.clone();
            let recorded = recorded.clone();
            tokio::spawn(async move {
                if let Ok(stream) = acceptor.accept(stream).await {
                    serve(stream, reply, recorded).await;
                }
            });
        }
    });
    let root = reqwest::Certificate::from_der(ca_cert.der()).unwrap();
    (
        Receiver {
            address,
            seen,
            task,
        },
        root,
    )
}

async fn serve(
    mut stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    reply: Reply,
    seen: Arc<Mutex<Vec<String>>>,
) {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    let (head_end, length) = loop {
        let read = match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(head_end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..head_end]).to_string();
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            break (head_end + 4, length);
        }
        if buffer.len() > 64 * 1024 {
            return;
        }
    };
    while buffer.len() < head_end + length {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
        }
    }
    seen.lock()
        .await
        .push(String::from_utf8_lossy(&buffer[head_end..]).to_string());
    if !reply.delay.is_zero() {
        tokio::time::sleep(reply.delay).await;
    }
    let mut response = format!(
        "HTTP/1.1 {} test\r\ncontent-length: 0\r\nconnection: close\r\n",
        reply.status
    );
    if let Some(after) = reply.retry_after {
        response.push_str(&format!("retry-after: {after}\r\n"));
    }
    response.push_str("\r\n");
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
    let _ = stream.shutdown().await;
}

struct PinnedResolver(SocketAddr);

#[async_trait::async_trait]
impl LogoutResolver for PinnedResolver {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, ()> {
        // Only the allowlisted origin reaches the fixture; any other name
        // resolves to a private address the policy must refuse.
        let ip = if host == HOST {
            self.0.ip()
        } else {
            "10.1.2.3".parse().unwrap()
        };
        Ok(vec![SocketAddr::new(ip, port)])
    }
}

fn policy(address: SocketAddr, timeout_millis: u64) -> Identity {
    Identity {
        logout_timeout_millis: timeout_millis,
        logout_private_origins: vec![format!("https://{HOST}:{}", address.port())],
        ..Default::default()
    }
}

fn transport(
    address: SocketAddr,
    timeout_millis: u64,
    roots: Vec<reqwest::Certificate>,
) -> HttpsLogoutTransport {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new(Arc::new(|| true)));
    HttpsLogoutTransport::with_resolver(
        policy(address, timeout_millis),
        clock,
        Arc::new(PinnedResolver(address)),
    )
    .with_extra_roots(roots)
    .with_loopback_fixture()
}

fn endpoint(address: SocketAddr) -> String {
    format!("https://{HOST}:{}/logout", address.port())
}

#[tokio::test]
async fn identity_logout_transport_delivers_over_https_and_repeats() {
    let (server, root) = receiver(Reply::status(204)).await;
    let delivery = transport(server.address, 2_000, vec![root]);
    let token = SecretToken::new("token-value".into()).unwrap();
    assert_eq!(
        delivery.send(&endpoint(server.address), &token).await,
        DeliveryOutcome::Delivered
    );
    assert_eq!(
        delivery.send(&endpoint(server.address), &token).await,
        DeliveryOutcome::Delivered
    );
    let seen = server.seen.lock().await.clone();
    assert_eq!(seen.len(), 2, "each attempt is a separate HTTPS request");
    for body in seen {
        assert_eq!(body, "logout_token=token-value");
    }
}

#[tokio::test]
async fn identity_logout_transport_rejects_untrusted_https_peer() {
    let (server, _root) = receiver(Reply::status(204)).await;
    let delivery = transport(server.address, 2_000, Vec::new());
    let token = SecretToken::new("token-value".into()).unwrap();
    assert_eq!(
        delivery.send(&endpoint(server.address), &token).await,
        DeliveryOutcome::Retry {
            error_code: "network_error"
        }
    );
    assert!(server.seen.lock().await.is_empty());
}

#[tokio::test]
async fn identity_logout_transport_maps_https_statuses() {
    let (server, root) = receiver(Reply {
        retry_after: Some("30"),
        ..Reply::status(429)
    })
    .await;
    let delivery = transport(server.address, 2_000, vec![root]);
    let token = SecretToken::new("token-value".into()).unwrap();
    assert_eq!(
        delivery.send(&endpoint(server.address), &token).await,
        DeliveryOutcome::RetryAfter {
            error_code: "http_429",
            retry_after_seconds: 30
        }
    );
    for (status, expected) in [
        (
            503,
            DeliveryOutcome::Retry {
                error_code: "http_5xx",
            },
        ),
        (
            400,
            DeliveryOutcome::PermanentFailure {
                error_code: "http_4xx",
            },
        ),
        (
            302,
            DeliveryOutcome::PermanentFailure {
                error_code: "http_redirect",
            },
        ),
    ] {
        let (server, root) = receiver(Reply::status(status)).await;
        let delivery = transport(server.address, 2_000, vec![root]);
        assert_eq!(
            delivery.send(&endpoint(server.address), &token).await,
            expected,
            "status {status}"
        );
    }
}

#[tokio::test]
async fn identity_logout_transport_times_out_slow_https_receiver() {
    let (server, root) = receiver(Reply {
        delay: Duration::from_millis(400),
        ..Reply::status(204)
    })
    .await;
    let delivery = transport(server.address, 120, vec![root]);
    let token = SecretToken::new("token-value".into()).unwrap();
    assert_eq!(
        delivery.send(&endpoint(server.address), &token).await,
        DeliveryOutcome::Retry {
            error_code: "timeout"
        }
    );
}

#[tokio::test]
async fn identity_logout_transport_only_reaches_the_allowlisted_origin() {
    let (server, root) = receiver(Reply::status(204)).await;
    let delivery = transport(server.address, 2_000, vec![root]);
    let token = SecretToken::new("token-value".into()).unwrap();
    let foreign = format!("https://other.example:{}/logout", server.address.port());
    assert_eq!(
        delivery.send(&foreign, &token).await,
        DeliveryOutcome::PermanentFailure {
            error_code: "ssrf_blocked"
        }
    );
    assert!(server.seen.lock().await.is_empty());
    assert_eq!(
        delivery.send(&endpoint(server.address), &token).await,
        DeliveryOutcome::Delivered
    );
}
