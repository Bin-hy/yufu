//! Capacity probes for the login, refresh, logout, delivery and cleanup paths.
//!
//! These are not part of the default suite: they run for minutes and print
//! numbers that only describe the machine they run on. Run them against the
//! deployment you plan to size, in a release build:
//!
//! ```text
//! cargo test --release --test mod capacity -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Keep `--test-threads=1`: every probe starts its own database container and
//! saturates the CPU, so running them together halves the numbers.
//!
//! Password verification is bcrypt cost 12 and saturates a core per request, so
//! login throughput scales with cores rather than with the client.

use super::super::common::*;
use super::auth::{LocalHttpFixture, local_http_fixture};
use auth::security::budget::Clock;
use auth::security::csrf::CsrfProtector;
use cobotusr::route::auth::{LoginData, RegistryData};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

const PASSWORD: &str = "capacity password";
const WINDOW: Duration = Duration::from_secs(5);

type Job =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = (Duration, Option<u16>)> + Send>> + Send + Sync>;

fn job<F, Fut>(f: F) -> Job
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = (Duration, Option<u16>)> + Send + 'static,
{
    Arc::new(move || Box::pin(f()))
}

fn system_clock() -> Arc<dyn auth::security::budget::Clock> {
    Arc::new(auth::security::budget::SystemClock::new(Arc::new(|| true)))
}

fn csrf(fixture: &LocalHttpFixture, refresh: Option<&str>) -> String {
    CsrfProtector::new("t24-local-test-key".into())
        .unwrap()
        .issue(&fixture.issuer, refresh, system_clock().as_ref())
        .unwrap()
        .expose_secret()
        .to_owned()
}

fn csrf_cookie(fixture: &LocalHttpFixture) -> &'static str {
    if fixture.insecure_cookies {
        "cobotusr-dev-csrf"
    } else {
        "__Host-cobotusr-csrf"
    }
}

async fn measure(label: &str, concurrency: usize, window: Duration, jobs: Vec<Job>) {
    assert_eq!(jobs.len(), concurrency);
    let deadline = Instant::now() + window;
    let mut workers = Vec::new();
    for job in jobs {
        workers.push(tokio::spawn(async move {
            let mut samples = Vec::new();
            let mut statuses = BTreeMap::new();
            while Instant::now() < deadline {
                let (elapsed, status) = job().await;
                samples.push(elapsed);
                *statuses.entry(status).or_insert(0usize) += 1;
            }
            (samples, statuses)
        }));
    }
    let mut samples = Vec::new();
    let mut statuses = BTreeMap::new();
    for worker in workers {
        let (mut worker_samples, worker_statuses) = worker.await.unwrap();
        samples.append(&mut worker_samples);
        for (status, count) in worker_statuses {
            *statuses.entry(status).or_insert(0usize) += count;
        }
    }
    samples.sort();
    let at = |fraction: f64| {
        samples[((samples.len() as f64 - 1.0) * fraction).round() as usize].as_millis()
    };
    println!(
        "{label:<26} {concurrency:>11} {:>8} {:>9.1} {:>6} {:>6} {:>6}  {statuses:?}",
        samples.len(),
        samples.len() as f64 / window.as_secs_f64(),
        at(0.5),
        at(0.95),
        samples.last().unwrap().as_millis(),
    );
}

fn header() {
    println!(
        "{:<26} {:>11} {:>8} {:>9} {:>6} {:>6} {:>6}  status",
        "probe", "concurrency", "requests", "req/s", "p50_ms", "p95_ms", "max_ms"
    );
}

async fn register(fixture: &LocalHttpFixture, username: &str) {
    let token = csrf(fixture, None);
    let response = fixture
        .client
        .post(format!("{}/register", fixture.api))
        .header("origin", &fixture.issuer)
        .header("x-csrf-token", &token)
        .header("cookie", format!("{}={token}", csrf_cookie(fixture)))
        .json(&RegistryData {
            username: username.into(),
            nickname: "capacity".into(),
            password: PASSWORD.into(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "register {username}");
}

/// One login request; returns the elapsed time, the status and the refresh
/// token the server set, if any.
async fn login(
    fixture: &LocalHttpFixture,
    username: &str,
) -> (Duration, Option<u16>, Option<String>) {
    let started = Instant::now();
    let token = csrf(fixture, None);
    let response = fixture
        .client
        .post(format!("{}/login", fixture.api))
        .header("origin", &fixture.issuer)
        .header("x-csrf-token", &token)
        .header("cookie", format!("{}={token}", csrf_cookie(fixture)))
        .json(&LoginData {
            username: username.into(),
            password: PASSWORD.into(),
        })
        .send()
        .await;
    let elapsed = started.elapsed();
    match response {
        Ok(response) => {
            let status = response.status().as_u16();
            let refresh = response
                .headers()
                .get_all("set-cookie")
                .iter()
                .filter_map(|value| value.to_str().ok())
                .find_map(|value| {
                    value
                        .strip_prefix("refresh_token=")
                        .and_then(|rest| rest.split(';').next())
                        .map(str::to_owned)
                });
            (elapsed, Some(status), refresh)
        }
        Err(_) => (elapsed, None, None),
    }
}

#[tokio::test]
#[ignore = "capacity probe; run explicitly with --ignored --nocapture"]
async fn capacity_probe_login_distinct_users() {
    let fixture = Arc::new(local_http_fixture("healthy").await);
    let users = 32;
    for index in 0..users {
        register(&fixture, &format!("capacity-{index}")).await;
    }
    header();
    for concurrency in [1usize, 4, 8, 16] {
        let jobs = (0..concurrency)
            .map(|worker| {
                let username = format!("capacity-{worker}");
                let fixture = fixture.clone();
                job(move || {
                    let username = username.clone();
                    let fixture = fixture.clone();
                    async move {
                        let (elapsed, status, _) = login(&fixture, &username).await;
                        (elapsed, status)
                    }
                })
            })
            .collect();
        measure("login (distinct users)", concurrency, WINDOW, jobs).await;
    }
}

/// Concurrent logins for one account serialize on the identity write barrier.
/// The loser reopens its snapshot and retries, so every request must succeed.
#[tokio::test]
#[ignore = "capacity probe; run explicitly with --ignored --nocapture"]
async fn capacity_probe_login_same_user() {
    let fixture = Arc::new(local_http_fixture("healthy").await);
    register(&fixture, "capacity-shared").await;
    header();
    for concurrency in [4usize, 16] {
        let jobs = (0..concurrency)
            .map(|_| {
                let fixture = fixture.clone();
                job(move || {
                    let fixture = fixture.clone();
                    async move {
                        let (elapsed, status, _) = login(&fixture, "capacity-shared").await;
                        (elapsed, status)
                    }
                })
            })
            .collect();
        measure("login (one account)", concurrency, WINDOW, jobs).await;
    }
}

#[tokio::test]
#[ignore = "capacity probe; run explicitly with --ignored --nocapture"]
async fn capacity_probe_refresh() {
    let fixture = Arc::new(local_http_fixture("healthy").await);
    let users = 16;
    let mut sessions = Vec::new();
    for index in 0..users {
        let username = format!("capacity-refresh-{index}");
        register(&fixture, &username).await;
        let (_, status, refresh) = login(&fixture, &username).await;
        assert_eq!(status, Some(200));
        sessions.push(refresh.expect("refresh cookie"));
    }
    // Each level continues from the token the previous level left behind; a
    // consumed refresh is a replay and would revoke the family.
    let sessions: Vec<_> = sessions
        .into_iter()
        .map(|token| Arc::new(std::sync::Mutex::new(token)))
        .collect();
    header();
    for concurrency in [1usize, 4, 8, 16] {
        let jobs = (0..concurrency)
            .map(|worker| {
                let fixture = fixture.clone();
                let current = sessions[worker].clone();
                job(move || {
                    let current = current.clone();
                    let fixture = fixture.clone();
                    async move {
                        let refresh = current.lock().unwrap().clone();
                        let token = csrf(&fixture, Some(&refresh));
                        let started = Instant::now();
                        let response = fixture
                            .client
                            .post(format!("{}/refresh", fixture.api))
                            .header("origin", &fixture.issuer)
                            .header("x-csrf-token", &token)
                            .header(
                                "cookie",
                                format!(
                                    "{}={token}; refresh_token={refresh}",
                                    csrf_cookie(&fixture)
                                ),
                            )
                            .send()
                            .await;
                        let elapsed = started.elapsed();
                        match response {
                            Ok(response) => {
                                let status = response.status().as_u16();
                                if status == 200
                                    && let Some(next) = response
                                        .headers()
                                        .get_all("set-cookie")
                                        .iter()
                                        .filter_map(|value| value.to_str().ok())
                                        .find_map(|value| {
                                            value
                                                .strip_prefix("refresh_token=")
                                                .and_then(|rest| rest.split(';').next())
                                                .map(str::to_owned)
                                        })
                                {
                                    *current.lock().unwrap() = next;
                                }
                                (elapsed, Some(status))
                            }
                            Err(_) => (elapsed, None),
                        }
                    }
                })
            })
            .collect();
        measure("refresh rotation", concurrency, WINDOW, jobs).await;
    }
}

/// Each logout revokes one session, so this measures the revocation path rather
/// than a repeatable loop: prepare one session per request, then fire them.
#[tokio::test]
#[ignore = "capacity probe; run explicitly with --ignored --nocapture"]
async fn capacity_probe_logout() {
    let fixture = Arc::new(local_http_fixture("healthy").await);
    let users = 32;
    header();
    for concurrency in [4usize, 16] {
        // A logout consumes its session, so every level needs its own batch.
        let mut sessions = Vec::new();
        for index in 0..users {
            let username = format!("capacity-logout-{concurrency}-{index}");
            register(&fixture, &username).await;
            let (_, status, refresh) = login(&fixture, &username).await;
            assert_eq!(status, Some(200));
            sessions.push(refresh.expect("refresh cookie"));
        }
        let started = Instant::now();
        let mut samples = Vec::new();
        let mut statuses = BTreeMap::new();
        let mut pending = sessions.clone().into_iter();
        let mut workers = Vec::new();
        for _ in 0..concurrency {
            let mut mine = Vec::new();
            while mine.len() < users / concurrency {
                if let Some(refresh) = pending.next() {
                    mine.push(refresh);
                } else {
                    break;
                }
            }
            let fixture = fixture.clone();
            workers.push(tokio::spawn(async move {
                let mut samples = Vec::new();
                let mut statuses = BTreeMap::new();
                for refresh in mine {
                    let token = csrf(&fixture, Some(&refresh));
                    let started = Instant::now();
                    let status = fixture
                        .client
                        .post(format!("{}/logout", fixture.api))
                        .header("origin", &fixture.issuer)
                        .header("x-csrf-token", &token)
                        .header(
                            "cookie",
                            format!("{}={token}; refresh_token={refresh}", csrf_cookie(&fixture)),
                        )
                        .send()
                        .await
                        .map(|response| response.status().as_u16())
                        .ok();
                    samples.push(started.elapsed());
                    *statuses.entry(status).or_insert(0usize) += 1;
                }
                (samples, statuses)
            }));
        }
        for worker in workers {
            let (mut worker_samples, worker_statuses) = worker.await.unwrap();
            samples.append(&mut worker_samples);
            for (status, count) in worker_statuses {
                *statuses.entry(status).or_insert(0usize) += count;
            }
        }
        samples.sort();
        let elapsed = started.elapsed();
        let at = |fraction: f64| {
            samples[((samples.len() as f64 - 1.0) * fraction).round() as usize].as_millis()
        };
        println!(
            "{:<26} {concurrency:>11} {:>8} {:>9.1} {:>6} {:>6} {:>6}  {statuses:?}",
            "logout (one-shot)",
            samples.len(),
            samples.len() as f64 / elapsed.as_secs_f64(),
            at(0.5),
            at(0.95),
            samples.last().unwrap().as_millis(),
        );
    }
}

struct Sink;

#[async_trait::async_trait]
impl cobotusr::service::identity::logout_dispatcher::LogoutTransport for Sink {
    async fn send(
        &self,
        _endpoint: &str,
        _token: &auth::security::identity::SecretToken,
    ) -> cobotusr::service::identity::repository_outbox::DeliveryOutcome {
        cobotusr::service::identity::repository_outbox::DeliveryOutcome::Delivered
    }
}

/// Backlog drain rate: the dispatcher leases ten deliveries per call and sends
/// four at a time, so the number to watch is deliveries per second, not the
/// per-call latency alone.
#[tokio::test]
#[ignore = "capacity probe; run explicitly with --ignored --nocapture"]
async fn capacity_probe_logout_delivery() {
    use cobotusr::domain::identity::session::IdentityRevocationScope;
    use cobotusr::service::identity::{
        logout_dispatcher::LogoutDispatcher, session::SessionService,
    };
    use uuid::Uuid;

    let f = IdentityFixture::start().await;
    let a = super::identity_support::app(&f, "capacity-delivery", true).await;
    let queued = 40;
    for _ in 0..queued {
        let (root, secret) = super::identity_support::center_secret(&f, a.owner).await;
        super::identity_support::identity_tokens(&f, &a, &root, "openid").await;
        SessionService::new(
            Arc::new(super::identity_support::repository(&f)),
            f.clock.clone(),
        )
        .revoke_personal(
            &secret,
            IdentityRevocationScope::CurrentCenter {
                sso_session_id: root.id,
            },
            Uuid::new_v4(),
            &super::identity_support::budget(&f),
        )
        .await
        .unwrap();
    }
    let dispatcher = LogoutDispatcher::new(
        Arc::new(super::identity_support::repository(&f)),
        f.clock.clone(),
        f.signer.clone(),
        f.config.clone(),
        Arc::new(Sink),
    )
    .unwrap();
    let started = Instant::now();
    let mut calls = 0usize;
    let mut delivered = 0usize;
    loop {
        let batch = dispatcher.dispatch_batch().await.unwrap();
        calls += 1;
        delivered += batch;
        if batch == 0 {
            break;
        }
    }
    let elapsed = started.elapsed();
    println!(
        "logout delivery backlog: {delivered} deliveries in {calls} batches, {:.1} deliveries/s, {:.1} ms/batch",
        delivered as f64 / elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1000.0 / calls as f64,
    );
    assert_eq!(delivered, queued);
}

/// Cleanup is bounded to one batch per table per call. The number to watch is
/// rows per second for a backlog that is already past its retention cutoffs.
#[tokio::test]
#[ignore = "capacity probe; run explicitly with --ignored --nocapture"]
async fn capacity_probe_cleanup() {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let f = IdentityFixture::start().await;
    let backlog = 2000i64;
    let at = f.clock.now().unwrap().unix_millis;
    let long_ago = at - 400 * 86_400_000;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"INSERT INTO identity_audit_event
            (id,request_id,action,object_kind,object_id,happened_at,result,change_summary,commit_marker)
           SELECT gen_random_uuid(),gen_random_uuid(),'capacity','probe','probe',$1,'success','{}'::jsonb,gen_random_uuid()
           FROM generate_series(1,$2)"#,
        [long_ago.into(), backlog.into()],
    ))
    .await
    .unwrap();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"INSERT INTO identity_request_operation
            (id,actor_id,operation_kind,idempotency_key,request_hash,resource_id,result_summary,committed_at)
           SELECT gen_random_uuid(),1,'capacity','capacity-'||g,repeat('a',64),'probe','{}'::jsonb,$1
           FROM generate_series(1,$2) AS g"#,
        [long_ago.into(), backlog.into()],
    ))
    .await
    .unwrap();
    let repository = super::identity_support::repository(&f);
    let started = Instant::now();
    let mut calls = 0usize;
    let mut deleted = 0u64;
    loop {
        let rows = repository
            .cleanup_identity(
                &super::identity_support::budget(&f),
                &f.config.identity,
                100,
            )
            .await
            .unwrap();
        calls += 1;
        deleted += rows;
        if rows == 0 {
            break;
        }
    }
    let elapsed = started.elapsed();
    println!(
        "cleanup backlog: {deleted} rows in {calls} calls, {:.0} rows/s, {:.1} ms/call",
        deleted as f64 / elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1000.0 / calls as f64,
    );
    assert!(deleted >= 2 * backlog as u64);
}
