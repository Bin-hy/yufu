//! Real cross-site OP, Web RP, SPA and protected API browser scenarios.
//!
//! The Rust side owns the OP, both applications, the reader credential and the
//! fixture certificate. The Node runner owns the two relying-party processes,
//! the TLS front ends and Playwright. Certificates come from the `openssl` CLI,
//! so the module is registered only on unix hosts.
use super::super::common::IdentityFixture;
use super::identity_support::*;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use uuid::Uuid;
use yufu::domain::identity::application::{AppEnvironmentKind, ClientKind};
use yufu::service::identity::reader::{ReaderLocation, ReaderService};

/// Binds an ephemeral port on a specific loopback address, then releases it so
/// the fixture can listen there. The window is small and the address is not
/// shared with any other test process.
fn free_port(host: &str) -> u16 {
    let listener = std::net::TcpListener::bind((host, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

/// macOS only aliases 127.0.0.1, so the second site listens dual-stack and is
/// addressed as `localhost`.
fn free_dual_stack_port() -> u16 {
    let listener = std::net::TcpListener::bind(("::", 0)).unwrap();
    listener.local_addr().unwrap().port()
}

fn openssl(args: &[&str]) {
    let output = std::process::Command::new("openssl")
        .args(args)
        .output()
        .expect("the browser SSO fixture requires the openssl CLI");
    assert!(
        output.status.success(),
        "openssl {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// One CA plus one server certificate valid for all three loopback sites.
fn certificates(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let ca_key = dir.join("ca.key");
    let ca = dir.join("ca.pem");
    let key = dir.join("server.key");
    let csr = dir.join("server.csr");
    let cert = dir.join("server.pem");
    let ext = dir.join("server.ext");
    std::fs::write(
        &ext,
        "subjectAltName=DNS:localhost,IP:127.0.0.1,IP:0:0:0:0:0:0:0:1\n\
         basicConstraints=critical,CA:FALSE\n\
         keyUsage=critical,digitalSignature,keyEncipherment\n\
         extendedKeyUsage=serverAuth\n",
    )
    .unwrap();
    let path = |p: &PathBuf| p.to_string_lossy().into_owned();
    openssl(&[
        "req",
        "-x509",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-keyout",
        &path(&ca_key),
        "-out",
        &path(&ca),
        "-days",
        "2",
        "-subj",
        "/CN=cobot-sso-fixture-ca",
        "-addext",
        "basicConstraints=critical,CA:TRUE",
    ]);
    openssl(&[
        "req",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-keyout",
        &path(&key),
        "-out",
        &path(&csr),
        "-subj",
        "/CN=cobot-sso-fixture",
    ]);
    openssl(&[
        "x509",
        "-req",
        "-in",
        &path(&csr),
        "-CA",
        &path(&ca),
        "-CAkey",
        &path(&ca_key),
        "-CAcreateserial",
        "-out",
        &path(&cert),
        "-days",
        "2",
        "-extfile",
        &path(&ext),
    ]);
    (ca, cert, key)
}

#[tokio::test]
async fn identity_browser_cross_site_three_engines_login_sso_and_logout() {
    let op_port = free_port("127.0.0.1");
    let a_port = free_dual_stack_port();
    let b_port = free_port("::1");
    let web_port = free_port("127.0.0.1");
    let api_port = free_port("127.0.0.1");
    let op_origin = format!("https://127.0.0.1:{op_port}");
    let a_origin = format!("https://localhost:{a_port}");
    let b_origin = format!("https://[::1]:{b_port}");
    let api_audience = "urn:app:spa-b";

    let issuer = op_origin.clone();
    let f = IdentityFixture::start_configured(|config| {
        // The browser sees the TLS front end, so the issuer must be that origin
        // even though the OP itself listens on plain HTTP behind the proxy.
        config.oidc.issuer = issuer.clone();
        // The three-engine suite runs for several minutes while the fixture
        // clock stays frozen at start-up. Keep the console access token valid
        // for the whole run instead of letting it expire mid-scenario.
        config.auth.exp_seconds = 3600;
    })
    .await;

    // Identity codes, flows and access tokens keep their production lifetimes,
    // but the fixture clock is frozen at start-up. Track the wall clock while
    // the browsers run so tokens minted minutes later are not already expired.
    let wall_clock = {
        let clock = f.clock.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                clock.set_wall_time(chrono::Utc::now().timestamp_millis() as u64);
            }
        })
    };

    let login_page = f
        .client
        .get(format!("{}/login", f.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        login_page.status(),
        200,
        "the embedded console must be built (pnpm build:web) and the webui feature enabled"
    );

    let web_origin = a_origin.clone();
    let _web = app_with_configs(
        &f,
        "web-a",
        ClientKind::ServerWeb,
        config_with(
            AppEnvironmentKind::Development,
            ClientKind::ServerWeb,
            true,
            |input| {
                input.redirect_uris = vec![format!("{web_origin}/callback")];
                input.post_logout_redirect_uris = vec![format!("{web_origin}/")];
                input.access_audience = "urn:app:web-a".into();
                input.backchannel_logout_uri =
                    Some(format!("{web_origin}/oidc/backchannel-logout"));
            },
        ),
        config_with(
            AppEnvironmentKind::Production,
            ClientKind::ServerWeb,
            true,
            |input| {
                input.redirect_uris = vec![format!("{web_origin}/callback")];
                input.post_logout_redirect_uris = vec![format!("{web_origin}/")];
                input.access_audience = "urn:app:web-a:production".into();
                input.backchannel_logout_uri =
                    Some(format!("{web_origin}/oidc/backchannel-logout"));
            },
        ),
    )
    .await;

    let spa_origin = b_origin.clone();
    let spa = app_with_configs(
        &f,
        "spa-b",
        ClientKind::BrowserSpa,
        config_with(
            AppEnvironmentKind::Development,
            ClientKind::BrowserSpa,
            false,
            |input| {
                input.redirect_uris = vec![format!("{spa_origin}/callback")];
                input.post_logout_redirect_uris = vec![format!("{spa_origin}/")];
                input.browser_origins = vec![spa_origin.clone()];
                input.access_audience = api_audience.into();
            },
        ),
        config_with(
            AppEnvironmentKind::Production,
            ClientKind::BrowserSpa,
            false,
            |input| {
                input.redirect_uris = vec![format!("{spa_origin}/callback")];
                input.post_logout_redirect_uris = vec![format!("{spa_origin}/")];
                input.browser_origins = vec![spa_origin.clone()];
                input.access_audience = "urn:app:spa-b:production".into();
            },
        ),
    )
    .await;

    let admin = admin_id(&f).await;
    let (_, center_secret_value) = center_secret_at(&f, &op_origin, admin).await;
    let browser_secret =
        yufu::domain::identity::account::IssuedSecret::from_entropy(rand::random()).into_response();
    let evidence = write_evidence_at(&f, &op_origin, Some(&center_secret_value), &browser_secret);
    let reader = ReaderService::new(Arc::new(repository(&f)), f.clock.clone())
        .create(
            &evidence,
            ReaderLocation {
                application_id: spa.app,
                environment_id: spa.development,
            },
            api_audience,
            "browser-sso-reader",
            Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    let reader_secret = reader
        .secret
        .expect("first reader create returns a secret")
        .into_response();

    let work = std::env::temp_dir().join(format!("cobot-sso-e2e-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&work).unwrap();
    let (ca, cert, key) = certificates(&work);
    let spa_dist = work.join("spa");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));

    let status = tokio::process::Command::new("node")
        .arg("tests/e2e/enterprise-sso-runner.mjs")
        .current_dir(repo.join("web"))
        .env("COBOT_SSO_REPO_ROOT", repo)
        .env("COBOT_SSO_OP_ORIGIN", &op_origin)
        .env("COBOT_SSO_A_ORIGIN", &a_origin)
        .env("COBOT_SSO_B_ORIGIN", &b_origin)
        .env("COBOT_SSO_OP_UPSTREAM", &f.base_url)
        .env(
            "COBOT_SSO_A_UPSTREAM",
            format!("http://127.0.0.1:{web_port}"),
        )
        .env(
            "COBOT_SSO_API_UPSTREAM",
            format!("http://127.0.0.1:{api_port}"),
        )
        .env("COBOT_SSO_CERT", &cert)
        .env("COBOT_SSO_KEY", &key)
        .env("COBOT_SSO_CA", &ca)
        .env("COBOT_SSO_SPA_DIST", &spa_dist)
        .env("COBOT_SSO_WEB_PORT", web_port.to_string())
        .env("COBOT_SSO_API_PORT", api_port.to_string())
        .env("COBOT_SSO_USERNAME", "identity-admin")
        .env("COBOT_SSO_PASSWORD", "fixture-admin-password")
        .env("COBOT_SSO_SUBJECT", admin.to_string())
        .env("COBOT_SSO_WEB_CLIENT_ID", "web-a-dev")
        .env("COBOT_SSO_WEB_CLIENT_SECRET", "fixture-client-secret")
        .env("COBOT_SSO_SPA_CLIENT_ID", "spa-b-dev")
        .env("COBOT_SSO_API_AUDIENCE", api_audience)
        .env("COBOT_SSO_READER_CLIENT_ID", &reader.reader_id)
        .env("COBOT_SSO_READER_CLIENT_SECRET", &reader_secret)
        .env(
            "COBOT_SSO_SESSION_SECRET",
            "fixture-session-secret-at-least-32-characters",
        )
        .env("COBOT_SSO_SIGNING_KEY", &f.key_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .unwrap();

    wall_clock.abort();

    let _ = std::fs::remove_dir_all(&work);
    f.close().await;
    assert!(
        status.success(),
        "three-engine browser SSO acceptance failed; see the Playwright output above"
    );
}
