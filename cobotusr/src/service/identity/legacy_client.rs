//! Explicit legacy machine compatibility, serialized with management/adoption.
use super::repository::{
    AuditEvent, AuditResult, IdentityLockKey, IdentityLockScope, IdentityTransaction,
    PgIdentityRepository, pg::get,
};
use crate::domain::identity::error::IdentityError;
use crate::service::security::{
    lifecycle::authenticate_session_client,
    repository::{PgSecurityTransaction, SecurityTransaction},
};
use crate::{
    AppState,
    entity::{application, oauth_client},
    error::AppError,
    result::Result,
};
use auth::security::{budget::SecurityBudget, error::SecurityError, identity::SecretToken};
use sea_orm::DatabaseTransaction;
use std::{future::Future, pin::Pin};
use uuid::Uuid;

type ClientFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// A sync target must already exist and belong to the same enabled enterprise
/// administrator as the confidential legacy client. Managed applications use
/// their explicit administration APIs instead.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute<T: Send, F>(
    state: &AppState,
    client_id: &str,
    secret: &SecretToken,
    sync_target: Option<&str>,
    grant: &str,
    request_id: Uuid,
    budget: &SecurityBudget,
    change: F,
) -> Result<T>
where
    F: for<'a> Fn(
            &'a DatabaseTransaction,
            &'a oauth_client::Model,
            Option<&'a application::Model>,
        ) -> ClientFuture<'a, T>
        + Send
        + Sync,
{
    let repo =
        PgIdentityRepository::new(state.database.connection.clone(), state.security_clock()?);
    for attempt in 0..3 {
        budget.remaining()?;
        let mut tx = match repo.security.begin_shared(budget).await {
            Ok(tx) => tx,
            Err(SecurityError::Conflict) if attempt < 2 => continue,
            Err(error) => return Err(error.into()),
        };
        let outcome = async {
            let (client, target) = lock_client(
                &mut tx,
                client_id,
                secret,
                sync_target,
                grant,
                budget,
                request_id,
            )
            .await?;
            let output = budget
                .run(change(&tx.transaction, &client, target.as_ref()))
                .await??;
            let event = Uuid::new_v4();
            tx.append_audit(&AuditEvent {
                id: event,
                request_id,
                actor_id: None,
                action: if sync_target.is_some() {
                    "legacy.permissions.sync"
                } else {
                    "legacy.client.issue"
                },
                object_kind: "oauth_client",
                object_id: client.id.to_string(),
                result: AuditResult::Success,
                reason: None,
                change_summary: serde_json::json!({}),
                operation_id: None,
                commit_marker: Some(event),
            })
            .await
            .map_err(|e| AppError::identity(e, request_id))?;
            budget.remaining()?;
            Ok::<_, AppError>(output)
        }
        .await;
        let output = match outcome {
            Ok(output) => output,
            Err(mut error) => {
                if let AppError::InternalServerError(inner) = &error
                    && let Some(db) = inner.downcast_ref::<sea_orm::DbErr>()
                    && crate::service::security::repository::unavailable(db.clone())
                        == SecurityError::Conflict
                {
                    error = SecurityError::Conflict.into();
                }
                let _ = Box::new(tx).rollback().await;
                if matches!(
                    error,
                    AppError::Security(SecurityError::Conflict)
                        | AppError::Identity {
                            reason: IdentityError::Conflict,
                            ..
                        }
                ) && attempt < 2
                {
                    continue;
                }
                return Err(error);
            }
        };
        match Box::new(tx).commit().await {
            Ok(()) => return Ok(output),
            Err(SecurityError::Conflict) if attempt < 2 => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(SecurityError::Conflict.into())
}

async fn lock_client(
    tx: &mut PgSecurityTransaction,
    client_id: &str,
    secret: &SecretToken,
    sync_target: Option<&str>,
    grant: &str,
    budget: &SecurityBudget,
    request_id: Uuid,
) -> Result<(oauth_client::Model, Option<application::Model>)> {
    tx.acquire_identity_scope(&IdentityLockScope::new(vec![IdentityLockKey::Policy])?)
        .await
        .map_err(|e| AppError::identity(e, request_id))?;
    let initial = tx
        .oidc_client(client_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid client authentication".into()))?;
    // Authenticate before resolving the sync target so an unauthenticated
    // caller cannot use the response to probe which application keys exist.
    authenticate_session_client(&initial, Some(secret), grant, budget)
        .await
        .map_err(|error| match error {
            SecurityError::InvalidCredential => {
                AppError::Unauthorized("invalid client authentication".into())
            }
            error => error.into(),
        })?;
    let mut keys = vec![IdentityLockKey::OAuthClient(initial.id)];
    let target = if let Some(app_key) = sync_target {
        let target = tx
            .run(application::Entity::get_by_app_key(
                &tx.transaction,
                app_key,
            ))
            .await?
            .ok_or_else(|| {
                AppError::BadRequest("application must be explicitly registered".into())
            })?;
        keys.push(IdentityLockKey::Application(target.id));
        let target_client_id = target.oidc_client_id.ok_or_else(|| {
            AppError::BadRequest("application must have an explicit legacy client binding".into())
        })?;
        keys.push(IdentityLockKey::OAuthClient(target_client_id));
        keys.push(IdentityLockKey::User(initial.user_id));
        Some(target)
    } else {
        None
    };
    tx.acquire_identity_scope(&IdentityLockScope::new(keys)?)
        .await
        .map_err(|e| AppError::identity(e, request_id))?;
    let client = tx
        .legacy_oidc_client(client_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid client authentication".into()))?;
    if client.client_secret_hash != initial.client_secret_hash
        || client.public_client != initial.public_client
    {
        return Err(SecurityError::InvalidCredential.into());
    }
    if let Some(target) = &target {
        let current = tx
            .run(application::Entity::get_by_app_key(
                &tx.transaction,
                &target.app_key,
            ))
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let row = tx
            .identity_one(
                "SELECT user_id,management_state FROM oauth_client WHERE id=$1",
                vec![current.oidc_client_id.into()],
            )
            .await
            .map_err(|e| AppError::identity(e, request_id))?;
        let actor = tx
            .identity_account(client.user_id)
            .await
            .map_err(|e| AppError::identity(e, request_id))?
            .ok_or(SecurityError::InvalidCredential)?;
        if current != *target
            || current.management_state != "legacy_unmanaged"
            || get::<String>(&row, "management_state")? != "legacy_unmanaged"
            || get::<i32>(&row, "user_id")? != client.user_id
            || !actor.enabled
            || !actor.administrator
        {
            return Err(SecurityError::Forbidden.into());
        }
    }
    Ok((client, target))
}
