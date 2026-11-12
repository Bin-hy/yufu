use super::super::common::IdentityFixture;
use auth::security::{
    budget::{Clock, SecurityBudget},
    identity::SecretToken,
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;
use yufu::domain::identity::{
    account::{ManagementMode, stable_principal},
    application::{
        AppEnvironmentKind, ClientKind, ConsentPolicy, EnvironmentConfig, EnvironmentConfigInput,
        LogoutCapability,
    },
    session::BrowserSessionSnapshot,
};
use yufu::service::identity::repository::*;

pub fn repository(f: &IdentityFixture) -> PgIdentityRepository {
    PgIdentityRepository::new(f.db.clone(), f.clock.clone())
}
pub fn budget(f: &IdentityFixture) -> SecurityBudget {
    SecurityBudget::new(f.clock.clone()).unwrap()
}
pub fn write_evidence(
    f: &IdentityFixture,
    center_secret: Option<&str>,
    browser_secret: &str,
) -> yufu::service::identity::browser::BrowserWriteEvidence {
    write_evidence_at(f, &f.base_url, center_secret, browser_secret)
}
/// Same evidence, but the request Origin is the issuer a browser actually sees.
/// The fixture listens on plain HTTP behind a TLS front end, so the two origins
/// differ by scheme and port.
pub fn write_evidence_at(
    f: &IdentityFixture,
    origin: &str,
    center_secret: Option<&str>,
    browser_secret: &str,
) -> yufu::service::identity::browser::BrowserWriteEvidence {
    use yufu::service::identity::browser;
    let mut request = axum::http::HeaderMap::new();
    request.insert(
        axum::http::header::COOKIE,
        format!(
            "yufu-dev-browser={browser_secret}{}",
            center_secret
                .map(|secret| format!("; yufu-dev-sso={secret}"))
                .unwrap_or_default()
        )
        .parse()
        .unwrap(),
    );
    let challenge = match center_secret {
        Some(secret) => browser::center_challenge(
            &f.config,
            f.clock.as_ref(),
            &request,
            &yufu::domain::identity::account::SecretDigest::of(secret),
        )
        .unwrap(),
        None => browser::anonymous_challenge(&f.config, f.clock.as_ref(), &request).unwrap(),
    };
    let original = request
        .get(axum::http::header::COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    let extra: Vec<_> = challenge
        .headers
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|h| h.to_str().unwrap().split(';').next().unwrap())
        .collect();
    request.insert(
        axum::http::header::COOKIE,
        format!("{original}; {}", extra.join("; ")).parse().unwrap(),
    );
    request.insert(axum::http::header::ORIGIN, origin.parse().unwrap());
    request.insert(
        "x-csrf-token",
        challenge.csrf_token.expose_secret().parse().unwrap(),
    );
    browser::verify_write(&f.config, f.clock.as_ref(), &request, None, &budget(f)).unwrap()
}
pub async fn scalar(f: &IdentityFixture, sql: &str) -> i64 {
    f.db.query_one_raw(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "value")
        .unwrap()
}
pub async fn admin_id(f: &IdentityFixture) -> i32 {
    i32::try_from(
        scalar(
            f,
            "SELECT id::bigint AS value FROM \"user\" WHERE username='identity-admin'",
        )
        .await,
    )
    .unwrap()
}
/// Old management routes are exercised with the same registered credentials
/// as the account center, so a signature-only fixture cannot mask a bypass.
pub async fn management_access(f: &IdentityFixture) -> String {
    let (root, _) = center_secret(f, admin_id(f).await).await;
    management_tokens(f, &root)
        .await
        .access_token
        .expose_secret()
        .into()
}
pub fn config(
    kind: AppEnvironmentKind,
    client: ClientKind,
    backchannel: bool,
) -> EnvironmentConfig {
    config_with(kind, client, backchannel, |_| {})
}
pub fn config_with(
    kind: AppEnvironmentKind,
    client: ClientKind,
    backchannel: bool,
    customize: impl FnOnce(&mut EnvironmentConfigInput),
) -> EnvironmentConfig {
    let mut input = EnvironmentConfigInput {
        client_kind: client,
        redirect_uris: vec!["https://rp.example/callback".into()],
        post_logout_redirect_uris: vec!["https://rp.example/bye".into()],
        browser_origins: if client == ClientKind::BrowserSpa {
            vec!["https://rp.example".into()]
        } else {
            vec![]
        },
        access_audience: format!("urn:test:{kind:?}"),
        identity_scopes: "openid profile email".into(),
        consent_policy: ConsentPolicy::UserConsent,
        logout_capability: if backchannel {
            LogoutCapability::BackchannelAndOnline
        } else {
            LogoutCapability::Online
        },
        backchannel_logout_uri: backchannel.then(|| "https://rp.example/backchannel".into()),
    };
    customize(&mut input);
    input.validate(kind, true).unwrap()
}
pub struct AppSeed {
    pub app: i32,
    pub development: Uuid,
    pub production: Uuid,
    pub dev_version: Uuid,
    pub prod_version: Uuid,
    pub owner: i32,
}
pub async fn app(f: &IdentityFixture, key: &str, backchannel: bool) -> AppSeed {
    app_kind(f, key, backchannel, ClientKind::ServerWeb).await
}

pub async fn app_kind(
    f: &IdentityFixture,
    key: &str,
    backchannel: bool,
    kind: ClientKind,
) -> AppSeed {
    app_with_configs(
        f,
        key,
        kind,
        config(AppEnvironmentKind::Development, kind, backchannel),
        config(AppEnvironmentKind::Production, kind, backchannel),
    )
    .await
}

pub async fn app_with_configs(
    f: &IdentityFixture,
    key: &str,
    kind: ClientKind,
    development: EnvironmentConfig,
    production: EnvironmentConfig,
) -> AppSeed {
    let owner = admin_id(f).await;
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE \"user\" SET developer_enabled=TRUE WHERE id=$1",
        [owner.into()],
    ))
    .await
    .unwrap();
    let dev = Uuid::new_v4();
    let prod = Uuid::new_v4();
    let dev_ver = Uuid::new_v4();
    let prod_ver = Uuid::new_v4();
    let secret_hash = kind
        .eq(&ClientKind::ServerWeb)
        .then(|| bcrypt::hash("fixture-client-secret", 4).unwrap());
    let mut tx = repository(f).begin(&budget(f)).await.unwrap();
    let write = RegistrationWrite {
        app_key: key.into(),
        name: key.into(),
        description: None,
        management_mode: ManagementMode::AdminConfigured,
        third_party: false,
        owner_user_id: owner,
        development: NewEnvironmentWrite {
            id: dev,
            client_id: format!("{key}-dev"),
            client_secret_hash: secret_hash
                .clone()
                .map(|hash| SecretToken::new(hash).unwrap()),
            initial_version_id: dev_ver,
            config: development,
        },
        production: NewEnvironmentWrite {
            id: prod,
            client_id: format!("{key}-prod"),
            client_secret_hash: secret_hash.map(|hash| SecretToken::new(hash).unwrap()),
            initial_version_id: prod_ver,
            config: production,
        },
    };
    let result = tx
        .apply(IdentityWrite::RegisterApplication(Box::new(write)))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let IdentityWriteResult::Application { application_id, .. } = result else {
        panic!("wrong receipt")
    };
    AppSeed {
        app: application_id,
        development: dev,
        production: prod,
        dev_version: dev_ver,
        prod_version: prod_ver,
        owner,
    }
}
pub async fn center(f: &IdentityFixture, user_id: i32) -> BrowserSessionSnapshot {
    center_secret(f, user_id).await.0
}

pub async fn center_secret(f: &IdentityFixture, user_id: i32) -> (BrowserSessionSnapshot, String) {
    center_secret_at(f, &f.base_url, user_id).await
}

/// The persisted principal namespace is the configured issuer, which differs
/// from the plain-HTTP listener when a TLS front end publishes another origin.
pub async fn center_secret_at(
    f: &IdentityFixture,
    issuer: &str,
    user_id: i32,
) -> (BrowserSessionSnapshot, String) {
    let secret = yufu::domain::identity::account::IssuedSecret::from_entropy(rand::random());
    let digest = secret.digest();
    let at = f.clock.now().unwrap().unix_millis;
    let root = BrowserSessionSnapshot {
        id: Uuid::new_v4(),
        principal: stable_principal(issuer, user_id).unwrap(),
        user_id,
        secret_digest: digest,
        auth_epoch: scalar(
            f,
            &format!("SELECT auth_epoch AS value FROM \"user\" WHERE id={user_id}"),
        )
        .await as u64,
        authenticated_at: at,
        expires_at: at + 43_200_000,
        revoked_at: None,
    };
    let mut tx = repository(f).begin(&budget(f)).await.unwrap();
    tx.apply(IdentityWrite::CreateCenter(root.clone()))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    (root, secret.into_response())
}

pub async fn identity_tokens(
    f: &IdentityFixture,
    a: &AppSeed,
    root: &BrowserSessionSnapshot,
    scopes: &str,
) -> (
    yufu::service::security::lifecycle::SessionTokenSet,
    yufu::domain::identity::session::IdentityTokenContext,
    Uuid,
) {
    identity_tokens_for_environment(f, a, root, scopes, a.development).await
}

pub async fn identity_tokens_for_environment(
    f: &IdentityFixture,
    a: &AppSeed,
    root: &BrowserSessionSnapshot,
    scopes: &str,
    environment_id: Uuid,
) -> (
    yufu::service::security::lifecycle::SessionTokenSet,
    yufu::domain::identity::session::IdentityTokenContext,
    Uuid,
) {
    use std::sync::Arc;
    use yufu::domain::identity::{application::IdentityScopeSet, session::IdentityTokenContext};
    use yufu::service::security::{
        lifecycle::CredentialLifecycle, repository::PgSecurityRepository,
    };
    let b = budget(f);
    let mut tx = repository(f).begin(&b).await.unwrap();
    tx.lock_identity_scope(
        &IdentityLockScope::new(vec![
            IdentityLockKey::Application(a.app),
            IdentityLockKey::Environment(environment_id),
            IdentityLockKey::User(a.owner),
            IdentityLockKey::User(root.user_id),
            IdentityLockKey::Center(root.id),
        ])
        .unwrap(),
    )
    .await
    .unwrap();
    tx.apply(IdentityWrite::RecomputeGeneration {
        environment_id,
        user_id: root.user_id,
    })
    .await
    .unwrap();
    let admission = tx
        .evaluate_admission(&AdmissionQuery {
            sso_session_id: root.id,
            environment_id,
            requested_scopes: IdentityScopeSet::parse(scopes).unwrap(),
        })
        .await
        .unwrap();
    let rp = Uuid::new_v4();
    let context = IdentityTokenContext {
        admission: admission.clone(),
        client_id: {
            let rows =
                f.db.query_one_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT client_id FROM oauth_client WHERE environment_id=$1",
                    [environment_id.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            rows.try_get("", "client_id").unwrap()
        },
        protocol_sid: Uuid::new_v4(),
        nonce: Some("request-specific-nonce".into()),
        root_expires_at: root.expires_at,
    };
    tx.apply(IdentityWrite::CreateRelyingParty {
        id: rp,
        protocol_sid: context.protocol_sid,
        admission: admission.clone(),
        nonce: context.nonce.clone(),
    })
    .await
    .unwrap();
    let profile = tx.load_profile(root.user_id).await.unwrap().unwrap();
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    let set = lifecycle
        .issue_identity_session_in_transaction(
            tx.as_mut(),
            &context,
            &profile,
            f.config.identity.access_token_ttl_seconds,
            f.signer.as_ref(),
            &b,
        )
        .await
        .unwrap();
    tx.apply(IdentityWrite::AttachIdentitySession {
        session_id: Uuid::parse_str(set.claims.session_id.as_ref().unwrap()).unwrap(),
        rp_session_id: rp,
        admission,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    (set, context, rp)
}

pub async fn management_tokens(
    f: &IdentityFixture,
    root: &BrowserSessionSnapshot,
) -> yufu::service::security::lifecycle::SessionTokenSet {
    management_tokens_for_user(f, root, "identity-admin", "fixture-admin-password").await
}

pub async fn management_tokens_for_user(
    f: &IdentityFixture,
    root: &BrowserSessionSnapshot,
    username: &str,
    plaintext: &str,
) -> yufu::service::security::lifecycle::SessionTokenSet {
    use std::sync::Arc;
    use yufu::service::security::{
        lifecycle::{CredentialLifecycle, LocalLoginRequest, LocalPassword, SessionTokenConfig},
        repository::PgSecurityRepository,
    };
    let b = budget(f);
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(f.db.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    let mut tx = repository(f).begin(&b).await.unwrap();
    let password = LocalPassword::new(plaintext.into()).unwrap();
    let session = lifecycle
        .login_local_in_transaction(
            tx.as_mut(),
            LocalLoginRequest {
                username,
                password: &password,
            },
            &SessionTokenConfig {
                issuer: f.base_url.clone(),
                access_ttl_seconds: 300,
                refresh_ttl_seconds: 3600,
            },
            f.signer.as_ref(),
        )
        .await
        .unwrap();
    tx.apply(IdentityWrite::AttachManagementSession {
        session_id: Uuid::parse_str(session.tokens.claims.session_id.as_ref().unwrap()).unwrap(),
        sso_session_id: root.id,
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    session.tokens
}
