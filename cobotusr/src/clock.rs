//! Host clock health, measured against configured NTP sources without setting time.
use crate::config::SecurityClock;
use auth::security::{
    budget::{Clock, ClockReading},
    error::SecurityError,
};
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::UdpSocket, task::JoinHandle, time::Instant};

const NTP_EPOCH: u64 = 2_208_988_800;
const MAX_ERROR_MILLIS: f64 = 1000.0;

#[derive(Clone, Copy)]
struct Sample {
    checked_at: Duration,
    unix_millis: u64,
    offset_millis: f64,
    uncertainty_millis: f64,
}

#[derive(serde::Serialize)]
pub struct ClockStatus {
    pub synchronized: bool,
    pub max_error_millis: Option<f64>,
    pub sample_age_millis: Option<u64>,
}

/// Owns the monitor task; dropping it stops probing and retained samples expire.
pub struct ClockMonitor {
    clock: Arc<MonitoredClock>,
    task: JoinHandle<()>,
}

impl ClockMonitor {
    pub async fn start(config: &SecurityClock) -> anyhow::Result<Self> {
        validate_config(config)?;
        let clock = Arc::new(MonitoredClock {
            origin: Instant::now(),
            ttl: Duration::from_secs(config.sample_ttl_seconds),
            sample: RwLock::new(None),
        });
        synchronize(&clock, config).await?;
        tracing::info!(max_error_millis = ?clock.status()?.max_error_millis, "security clock synchronized");
        let owned_config = config.clone();
        let monitored = clock.clone();
        let task = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(owned_config.poll_interval_seconds));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            interval.tick().await;
            loop {
                interval.tick().await;
                let was_healthy = monitored.status().is_ok_and(|status| status.synchronized);
                if let Err(error) = synchronize(&monitored, &owned_config).await {
                    tracing::warn!(reason = %error, "security clock unhealthy; authentication unavailable");
                } else if !was_healthy {
                    tracing::info!("security clock recovered");
                }
            }
        });
        Ok(Self { clock, task })
    }

    pub fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    pub fn status(&self) -> Result<ClockStatus, SecurityError> {
        self.clock.status()
    }
}

impl Drop for ClockMonitor {
    fn drop(&mut self) {
        self.task.abort();
        if let Ok(mut sample) = self.clock.sample.write() {
            *sample = None;
        }
    }
}

struct MonitoredClock {
    origin: Instant,
    ttl: Duration,
    sample: RwLock<Option<Sample>>,
}

fn unix_millis() -> Result<u64, SecurityError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|time| u64::try_from(time.as_millis()).ok())
        .ok_or(SecurityError::DependencyUnavailable)
}

impl MonitoredClock {
    fn status_at(&self, monotonic: Duration, wall: u64) -> Result<ClockStatus, SecurityError> {
        let sample = *self
            .sample
            .read()
            .map_err(|_| SecurityError::DependencyUnavailable)?;
        let Some(sample) = sample else {
            return Ok(ClockStatus {
                synchronized: false,
                max_error_millis: None,
                sample_age_millis: None,
            });
        };
        let age = monotonic.checked_sub(sample.checked_at);
        let age_ms = age.and_then(|age| u64::try_from(age.as_millis()).ok());
        // A wall-clock jump or process suspension cannot extend a cached health result.
        let jump = age_ms.map(|elapsed| {
            ((i128::from(wall) - i128::from(sample.unix_millis)) - i128::from(elapsed))
                .unsigned_abs() as f64
        });
        let bound = jump.map(|jump| sample.offset_millis.abs() + sample.uncertainty_millis + jump);
        Ok(ClockStatus {
            synchronized: age.is_some_and(|age| age < self.ttl)
                && bound.is_some_and(|bound| bound <= MAX_ERROR_MILLIS),
            max_error_millis: bound,
            sample_age_millis: age_ms,
        })
    }

    fn status(&self) -> Result<ClockStatus, SecurityError> {
        self.status_at(self.origin.elapsed(), unix_millis()?)
    }
}

impl Clock for MonitoredClock {
    fn now(&self) -> Result<ClockReading, SecurityError> {
        let monotonic = self.origin.elapsed();
        let wall = unix_millis()?;
        Ok(ClockReading {
            monotonic,
            unix_millis: wall,
            synchronized: self.status_at(monotonic, wall)?.synchronized,
        })
    }
}

fn validate_config(config: &SecurityClock) -> anyhow::Result<()> {
    anyhow::ensure!(
        !config.servers.is_empty() && config.servers.len() <= 8,
        "security_clock.servers must contain 1 to 8 trusted NTP addresses"
    );
    anyhow::ensure!(
        config.servers.iter().collect::<HashSet<_>>().len() == config.servers.len(),
        "security_clock.servers must be unique"
    );
    anyhow::ensure!(
        config.min_agreeing_sources > 0 && config.min_agreeing_sources <= config.servers.len(),
        "invalid security_clock.min_agreeing_sources"
    );
    anyhow::ensure!(
        (1..=30).contains(&config.poll_interval_seconds)
            && config.sample_ttl_seconds > config.poll_interval_seconds
            && config.sample_ttl_seconds <= 60,
        "invalid security_clock polling or sample lifetime"
    );
    anyhow::ensure!(
        (100..=2000).contains(&config.timeout_millis),
        "security_clock.timeout_millis must be between 100 and 2000"
    );
    Ok(())
}

async fn synchronize(clock: &MonitoredClock, config: &SecurityClock) -> anyhow::Result<()> {
    let results = futures::future::join_all(config.servers.iter().map(|server| async {
        tokio::time::timeout(
            Duration::from_millis(config.timeout_millis),
            probe(clock, server),
        )
        .await
        .ok()
        .and_then(Result::ok)
    }))
    .await;
    // DNS aliases of one source cannot supply multiple votes.
    let mut sources = HashSet::new();
    let samples: Vec<_> = results
        .into_iter()
        .flatten()
        .filter_map(|(address, sample)| sources.insert(address.ip()).then_some(sample))
        .collect();
    let selected = select_sample(&samples, config.min_agreeing_sources);
    *clock
        .sample
        .write()
        .map_err(|_| anyhow::anyhow!("clock health state unavailable"))? = selected;
    anyhow::ensure!(
        selected.is_some(),
        "NTP clock health could not be confirmed within 1 second; check trusted servers, UDP connectivity and OS time synchronization"
    );
    Ok(())
}

fn select_sample(samples: &[Sample], quorum: usize) -> Option<Sample> {
    // A quorum needs a common interval, not merely overlaps with one wide sample.
    let agreeing: Vec<_> = samples
        .iter()
        .flat_map(|sample| {
            let point = sample.offset_millis - sample.uncertainty_millis;
            let peers: Vec<_> = samples
                .iter()
                .filter(|peer| {
                    peer.offset_millis - peer.uncertainty_millis <= point
                        && point <= peer.offset_millis + peer.uncertainty_millis
                })
                .collect();
            if peers.len() >= quorum { peers } else { vec![] }
        })
        .collect();
    agreeing
        .into_iter()
        .min_by(|left, right| {
            (left.offset_millis.abs() + left.uncertainty_millis)
                .total_cmp(&(right.offset_millis.abs() + right.uncertainty_millis))
        })
        .copied()
}

async fn probe(
    clock: &MonitoredClock,
    server: &str,
) -> anyhow::Result<(std::net::SocketAddr, Sample)> {
    let address = tokio::net::lookup_host(server)
        .await?
        .next()
        .ok_or_else(|| anyhow::anyhow!("NTP address unavailable"))?;
    let socket = UdpSocket::bind(if address.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .await?;
    socket.connect(address).await?;
    let sent_wall = unix_millis()?;
    let sent_mono = clock.origin.elapsed();
    let mut request = [0u8; 48];
    request[0] = 0x23; // NTP v4, client mode.
    let seconds = sent_wall / 1000 + NTP_EPOCH;
    let fraction = ((sent_wall % 1000) << 32) / 1000;
    // Randomize sub-millisecond bits so a pending response must echo a fresh nonce.
    let stamp = ((seconds << 32) | fraction) ^ u64::from(rand::random::<u16>());
    request[40..48].copy_from_slice(&stamp.to_be_bytes());
    socket.send(&request).await?;
    let mut buffer = [0u8; 512];
    let size = socket.recv(&mut buffer).await?;
    let received_wall = unix_millis()?;
    let received_mono = clock.origin.elapsed();
    let (offset_millis, uncertainty_millis) = parse_response(
        &buffer[..size],
        &request[40..48],
        sent_wall,
        received_wall,
        (received_mono - sent_mono).as_secs_f64() * 1000.0,
    )?;
    Ok((
        address,
        Sample {
            checked_at: received_mono,
            unix_millis: received_wall,
            offset_millis,
            uncertainty_millis,
        },
    ))
}

fn ntp_millis(bytes: &[u8], reference_wall: u64) -> anyhow::Result<f64> {
    anyhow::ensure!(
        bytes.len() == 8 && bytes.iter().any(|byte| *byte != 0),
        "invalid NTP timestamp"
    );
    let seconds = u32::from_be_bytes(bytes[..4].try_into()?);
    let fraction = u32::from_be_bytes(bytes[4..].try_into()?);
    let reference = i128::from(reference_wall / 1000 + NTP_EPOCH);
    let era = (reference - i128::from(seconds) + (1i128 << 31)).div_euclid(1i128 << 32);
    Ok(
        ((era * (1i128 << 32) + i128::from(seconds) - i128::from(NTP_EPOCH)) as f64
            + f64::from(fraction) / 4_294_967_296.0)
            * 1000.0,
    )
}

fn parse_response(
    packet: &[u8],
    nonce: &[u8],
    sent: u64,
    received: u64,
    monotonic_rtt: f64,
) -> anyhow::Result<(f64, f64)> {
    anyhow::ensure!(
        packet.len() >= 48
            && packet[0] >> 6 != 3
            && matches!((packet[0] >> 3) & 7, 3 | 4)
            && packet[0] & 7 == 4
            && (1..=15).contains(&packet[1]),
        "NTP server is not synchronized"
    );
    anyhow::ensure!(
        &packet[24..32] == nonce,
        "NTP response does not match the pending request"
    );
    let received_at_server = ntp_millis(&packet[32..40], sent)?;
    let sent_at_server = ntp_millis(&packet[40..48], received)?;
    let wall_rtt = (i128::from(received) - i128::from(sent)) as f64;
    anyhow::ensure!(
        (wall_rtt - monotonic_rtt).abs() < 10.0,
        "system time changed during NTP probe"
    );
    let processing = sent_at_server - received_at_server;
    anyhow::ensure!(
        processing >= 0.0 && processing <= monotonic_rtt + 2.0,
        "invalid NTP processing timestamps"
    );
    let root_delay = f64::from(i32::from_be_bytes(packet[4..8].try_into()?)) * 1000.0 / 65_536.0;
    let dispersion = f64::from(u32::from_be_bytes(packet[8..12].try_into()?)) * 1000.0 / 65_536.0;
    anyhow::ensure!(root_delay >= 0.0, "invalid NTP root delay");
    let uncertainty =
        (monotonic_rtt - processing).max(0.0) / 2.0 + root_delay / 2.0 + dispersion + 2.0;
    let offset = ((received_at_server - sent as f64) + (sent_at_server - received as f64)) / 2.0;
    anyhow::ensure!(
        offset.abs() + uncertainty <= MAX_ERROR_MILLIS,
        "NTP clock error exceeds the 1 second deployment bound"
    );
    Ok((offset, uncertainty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth::security::budget::SecurityBudget;
    use std::sync::atomic::{AtomicI64, Ordering};

    fn timestamp(wall: u64) -> [u8; 8] {
        let seconds = wall / 1000 + NTP_EPOCH;
        let fraction = ((wall % 1000) << 32) / 1000;
        ((seconds << 32) | fraction).to_be_bytes()
    }

    fn packet(wall: u64, nonce: &[u8]) -> [u8; 48] {
        let mut packet = [0u8; 48];
        packet[0] = 0x24;
        packet[1] = 1;
        packet[24..32].copy_from_slice(nonce);
        packet[32..40].copy_from_slice(&timestamp(wall));
        packet[40..48].copy_from_slice(&timestamp(wall));
        packet
    }

    #[test]
    fn test_ntp_response_checks_nonce_health_error_bound_and_round_trip() {
        let sent = 1_800_000_000_000;
        let nonce = timestamp(sent);
        let valid = packet(sent + 150, &nonce);
        let (offset, uncertainty) =
            parse_response(&valid, &nonce, sent, sent + 100, 100.0).unwrap();
        assert!((offset - 100.0).abs() < 1.0);
        assert!(uncertainty >= 50.0);
        assert!(parse_response(&valid[..47], &nonce, sent, sent + 100, 100.0).is_err());
        assert!(parse_response(&valid, &[9; 8], sent, sent + 100, 100.0).is_err());
        for (index, value) in [(0, 0xe4), (0, 0x23), (1, 0), (1, 16)] {
            let mut bad = valid;
            bad[index] = value;
            assert!(parse_response(&bad, &nonce, sent, sent + 100, 100.0).is_err());
        }
        assert!(
            parse_response(
                &packet(sent + 1500, &nonce),
                &nonce,
                sent,
                sent + 100,
                100.0
            )
            .is_err()
        );
        assert!(parse_response(&valid, &nonce, sent, sent + 500, 100.0).is_err());
        assert!(parse_response(&valid, &nonce, sent, sent + 2200, 2200.0).is_err());
    }

    #[test]
    fn test_ntp_timestamp_supports_2036_era_rollover() {
        for wall in [1_800_000_000_123, 2_085_978_500_456, 2_200_000_000_789] {
            assert!((ntp_millis(&timestamp(wall), wall).unwrap() - wall as f64).abs() < 1.0);
        }
    }

    fn sample() -> Sample {
        Sample {
            checked_at: Duration::from_secs(10),
            unix_millis: 1_000_000,
            offset_millis: 100.0,
            uncertainty_millis: 50.0,
        }
    }

    #[test]
    fn test_clock_health_expires_and_detects_wall_jumps_and_monotonic_rollback() {
        let clock = MonitoredClock {
            origin: Instant::now(),
            ttl: Duration::from_secs(30),
            sample: RwLock::new(Some(sample())),
        };
        assert!(
            clock
                .status_at(Duration::from_secs(11), 1_001_000)
                .unwrap()
                .synchronized
        );
        assert!(
            !clock
                .status_at(Duration::from_secs(40), 1_030_000)
                .unwrap()
                .synchronized
        );
        assert!(
            !clock
                .status_at(Duration::from_secs(11), 1_003_000)
                .unwrap()
                .synchronized
        );
        assert!(
            !clock
                .status_at(Duration::from_secs(11), 999_000)
                .unwrap()
                .synchronized
        );
        assert!(
            !clock
                .status_at(Duration::from_secs(9), 1_000_000)
                .unwrap()
                .synchronized
        );
        *clock.sample.write().unwrap() = None;
        assert!(
            !clock
                .status_at(Duration::from_secs(11), 1_001_000)
                .unwrap()
                .synchronized
        );
    }

    #[test]
    fn test_clock_quorum_rejects_single_and_disagreeing_sources() {
        let first = sample();
        let matching = Sample {
            offset_millis: 110.0,
            ..first
        };
        let divergent = Sample {
            offset_millis: 500.0,
            ..first
        };
        assert!(select_sample(&[first], 2).is_none());
        assert!(select_sample(&[first, divergent], 2).is_none());
        assert!(select_sample(&[first, matching, divergent], 2).is_some());
        let wide = Sample {
            uncertainty_millis: 200.0,
            ..first
        };
        let left = Sample {
            offset_millis: -50.0,
            uncertainty_millis: 10.0,
            ..first
        };
        let right = Sample {
            offset_millis: 250.0,
            uncertainty_millis: 10.0,
            ..first
        };
        assert!(select_sample(&[wide, left, right], 3).is_none());
    }

    #[test]
    fn test_clock_config_rejects_missing_duplicate_and_unbounded_settings() {
        assert!(validate_config(&SecurityClock::default()).is_ok());
        assert!(
            validate_config(&SecurityClock {
                servers: vec![],
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_config(&SecurityClock {
                servers: vec!["same:123".into(); 2],
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_config(&SecurityClock {
                sample_ttl_seconds: 1000,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_config(&SecurityClock {
                timeout_millis: 10_000,
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_config(&SecurityClock {
                min_agreeing_sources: 4,
                ..Default::default()
            })
            .is_err()
        );
    }

    async fn ntp_fixture() -> (String, Arc<AtomicI64>, JoinHandle<()>) {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap().to_string();
        let offset = Arc::new(AtomicI64::new(0));
        let active_offset = offset.clone();
        let task = tokio::spawn(async move {
            let mut buffer = [0u8; 512];
            while let Ok((size, sender)) = socket.recv_from(&mut buffer).await {
                if size < 48 {
                    continue;
                }
                let wall =
                    (unix_millis().unwrap() as i64 + active_offset.load(Ordering::SeqCst)) as u64;
                socket
                    .send_to(&packet(wall, &buffer[40..48]), sender)
                    .await
                    .unwrap();
            }
        });
        (address, offset, task)
    }

    #[tokio::test]
    async fn test_monitor_uses_real_udp_samples_and_invalidates_on_drop() {
        let (server, _, task) = ntp_fixture().await;
        let monitor = ClockMonitor::start(&SecurityClock {
            servers: vec![server],
            min_agreeing_sources: 1,
            timeout_millis: 100,
            ..Default::default()
        })
        .await
        .unwrap();
        let clock = monitor.clock();
        assert!(monitor.status().unwrap().synchronized);
        assert!(SecurityBudget::new(clock.clone()).is_ok());
        drop(monitor);
        assert!(SecurityBudget::new(clock).is_err());
        task.abort();
    }

    #[tokio::test]
    async fn test_monitor_failed_probe_revokes_health_and_later_recovers() {
        let (server, offset, task) = ntp_fixture().await;
        let config = SecurityClock {
            servers: vec![server],
            min_agreeing_sources: 1,
            timeout_millis: 100,
            ..Default::default()
        };
        let monitor = ClockMonitor::start(&config).await.unwrap();
        offset.store(5000, Ordering::SeqCst);
        assert!(synchronize(&monitor.clock, &config).await.is_err());
        assert!(SecurityBudget::new(monitor.clock()).is_err());
        offset.store(0, Ordering::SeqCst);
        synchronize(&monitor.clock, &config).await.unwrap();
        assert!(SecurityBudget::new(monitor.clock()).is_ok());
        task.abort();
    }

    #[tokio::test]
    async fn test_monitor_unavailable_source_fails_startup_with_diagnostic() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap().to_string();
        let error = ClockMonitor::start(&SecurityClock {
            servers: vec![address],
            min_agreeing_sources: 1,
            timeout_millis: 100,
            ..Default::default()
        })
        .await
        .err()
        .unwrap();
        assert!(
            error
                .to_string()
                .contains("NTP clock health could not be confirmed")
        );
    }

    #[tokio::test]
    async fn test_monitor_dns_aliases_cannot_supply_multiple_clock_votes() {
        let (server, _, task) = ntp_fixture().await;
        let alias = server.replacen("127.0.0.1", "localhost", 1);
        assert!(
            ClockMonitor::start(&SecurityClock {
                servers: vec![server, alias],
                min_agreeing_sources: 2,
                timeout_millis: 100,
                ..Default::default()
            })
            .await
            .is_err()
        );
        task.abort();
    }
}
