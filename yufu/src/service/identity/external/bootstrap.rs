//! Optional environment bootstrap for single-binary deployments before the admin UI exists.
//! Existing stored credentials are never overwritten.
use super::SecretCipher;
use crate::domain::identity::error::IdentityError;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

const CLIENT_ID: &str = "YUFU_EXTERNAL_GITHUB_CLIENT_ID";
const CLIENT_SECRET: &str = "YUFU_EXTERNAL_GITHUB_CLIENT_SECRET";
const ENABLED: &str = "YUFU_EXTERNAL_GITHUB_ENABLED";

pub async fn bootstrap_github(
    db: &DatabaseConnection,
    cipher: Option<&SecretCipher>,
    now_millis: u64,
) -> Result<(), IdentityError> {
    let client_id = std::env::var(CLIENT_ID)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let client_secret = std::env::var(CLIENT_SECRET)
        .ok()
        .filter(|value| !value.trim().is_empty());
    if client_id.is_none() && client_secret.is_none() {
        return Ok(());
    }
    let (client_id, client_secret) = match (client_id, client_secret) {
        (Some(client_id), Some(client_secret)) => (client_id, client_secret),
        _ => return Err(IdentityError::DependencyUnavailable),
    };
    let cipher = cipher.ok_or(IdentityError::DependencyUnavailable)?;
    let ciphertext = cipher.encrypt("github", &client_secret)?;
    let enabled = std::env::var(ENABLED)
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes"));
    let at = i64::try_from(now_millis).map_err(|_| IdentityError::InvalidInput)?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE identity_external_provider SET client_id=CASE WHEN client_id='' THEN $1 ELSE client_id END, secret_ciphertext=COALESCE(secret_ciphertext,$2), enabled=CASE WHEN $3 THEN TRUE ELSE enabled END, revision=revision+1, updated_at=$4 WHERE provider_key='github' AND (client_id='' OR secret_ciphertext IS NULL OR ($3 AND NOT enabled))",
        [client_id.into(), ciphertext.into(), enabled.into(), at.into()],
    ))
    .await
    .map_err(|_| IdentityError::DependencyUnavailable)?;
    Ok(())
}
