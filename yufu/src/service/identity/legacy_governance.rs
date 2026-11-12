//! Compatibility writes share the primary security transaction. The policy
//! barrier serializes role membership/definition changes; affected user row
//! updates also invalidate concurrent identity admission snapshots.
use super::repository::{pg::get, *};
use crate::domain::identity::{
    account::validate_account_change, error::IdentityError, session::IdentityRevocationScope,
};
use crate::service::security::{
    browser,
    lifecycle::status_for_snapshot,
    repository::{PgSecurityTransaction, SecurityTransaction, now},
};
use crate::{AppState, error::AppError, result::Result};
use auth::security::{
    budget::SecurityBudget,
    credential::AuthenticatedContext,
    error::SecurityError,
    policy::{PolicyEffect, evaluate_policy},
};
use sea_orm::DatabaseTransaction;
use std::{collections::BTreeMap, future::Future, pin::Pin};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) enum MutationScope {
    User(i32),
    Role(i32),
    Permission(i32),
    Policy,
    Registry,
    Application { id: i32, affects_policy: bool },
    Client(i32),
    ClientConsent(i32),
}
type MutationFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub(crate) async fn mutate<T: Send, F>(
    state: &AppState,
    context: &LegacyRequest,
    method: &'static str,
    route: &'static str,
    scope: MutationScope,
    change: F,
) -> Result<T>
where
    F: for<'a> Fn(&'a DatabaseTransaction) -> MutationFuture<'a, T> + Send + Sync,
{
    let budget = context.budget.clone();
    let manager = &context.manager;
    let max_users = state.config.identity.legacy_governance_max_users;
    let repository =
        PgIdentityRepository::new(state.database.connection.clone(), state.security_clock()?);
    let request_id = context.id;
    for attempt in 0..3 {
        budget.remaining()?;
        let mut tx = match repository.security.begin_shared(&budget).await {
            Ok(tx) => tx,
            Err(SecurityError::Conflict) if attempt < 2 => continue,
            Err(error) => return Err(error.into()),
        };
        let output = match apply(
            &mut tx, manager, method, route, scope, request_id, max_users, &change,
        )
        .await
        {
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
                if retryable(&error) && attempt < 2 {
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
fn retryable(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Security(SecurityError::Conflict)
            | AppError::Identity {
                reason: IdentityError::Conflict,
                ..
            }
    )
}
// The bound, the scope and the mutation callback are all part of the same
// governance contract; bundling them into a struct would only rename the list.
#[allow(clippy::too_many_arguments)]
async fn apply<T, F>(
    tx: &mut PgSecurityTransaction,
    manager: &AuthenticatedContext,
    method: &str,
    route: &str,
    scope: MutationScope,
    request_id: Uuid,
    max_users: usize,
    change: &F,
) -> Result<T>
where
    F: for<'a> Fn(&'a DatabaseTransaction) -> MutationFuture<'a, T> + Send + Sync,
{
    tx.acquire_identity_scope(
        &IdentityLockScope::new(vec![IdentityLockKey::Policy])
            .map_err(|e| AppError::identity(e, request_id))?,
    )
    .await
    .map_err(|e| AppError::identity(e, request_id))?;
    let (predicate, values): (&str, Vec<sea_orm::Value>) = match scope {
        MutationScope::User(id) if id > 0 => ("u.id=$1", vec![id.into()]),
        MutationScope::Role(id) if id > 0 => ("u.role_id=$1", vec![id.into()]),
        MutationScope::Permission(id) if id > 0 => (
            "EXISTS (SELECT 1 FROM role_permission p WHERE p.role_id=u.role_id AND p.permission_id=$1) OR EXISTS (SELECT 1 FROM user_permission p WHERE p.user_id=u.id AND p.permission_id=$1)",
            vec![id.into()],
        ),
        MutationScope::Policy => ("TRUE", vec![]),
        MutationScope::Registry => ("FALSE", vec![]),
        MutationScope::Application {
            id,
            affects_policy: true,
        } if id > 0 => (
            "EXISTS (SELECT 1 FROM permission p WHERE p.application_id=$1 AND (EXISTS (SELECT 1 FROM role_permission rp WHERE rp.role_id=u.role_id AND rp.permission_id=p.id) OR EXISTS (SELECT 1 FROM user_permission up WHERE up.user_id=u.id AND up.permission_id=p.id)))",
            vec![id.into()],
        ),
        MutationScope::Application {
            id,
            affects_policy: false,
        } if id > 0 => ("FALSE", vec![]),
        MutationScope::Client(id) | MutationScope::ClientConsent(id) if id > 0 => ("FALSE", vec![]),
        _ => return Err(AppError::identity(IdentityError::InvalidInput, request_id)),
    };
    let rows = tx
        .identity_rows(
            &format!(
                "SELECT u.id FROM \"user\" u WHERE {predicate} ORDER BY u.id LIMIT {}",
                max_users.saturating_add(1)
            ),
            values,
        )
        .await
        .map_err(|e| AppError::identity(e, request_id))?;
    if rows.len() > max_users {
        return Err(AppError::identity(
            IdentityError::CapacityExceeded,
            request_id,
        ));
    }
    let mut before = BTreeMap::new();
    let mut keys = vec![IdentityLockKey::Policy];
    match scope {
        MutationScope::Application { id, .. } => keys.push(IdentityLockKey::Application(id)),
        MutationScope::Client(id) | MutationScope::ClientConsent(id) => {
            keys.push(IdentityLockKey::OAuthClient(id));
            for row in tx.identity_rows("SELECT id AS application_id,NULL::uuid AS environment_id FROM application WHERE oidc_client_id=$1 UNION SELECT application_id,id AS environment_id FROM identity_app_environment WHERE oauth_client_id=$1",vec![id.into()]).await.map_err(|e|AppError::identity(e,request_id))? {
                keys.push(IdentityLockKey::Application(get(&row,"application_id").map_err(|e|AppError::identity(e,request_id))?));
                if let Some(environment_id) = get::<Option<Uuid>>(&row,"environment_id").map_err(|e|AppError::identity(e,request_id))? {
                    keys.push(IdentityLockKey::Environment(environment_id));
                }
            }
        }
        _ => {}
    }
    let actor_id = manager
        .principal()
        .subject()
        .parse::<i32>()
        .map_err(|_| SecurityError::InvalidCredential)?;
    for row in rows {
        let id = get(&row, "id").map_err(|e| AppError::identity(e, request_id))?;
        let snapshot = tx
            .account_governance(id)
            .await
            .map_err(|e| AppError::identity(e, request_id))?;
        let plan = tx
            .revocation_plan(&IdentityRevocationScope::UserAll { user_id: id })
            .await
            .map_err(|e| AppError::identity(e, request_id))?;
        keys.extend_from_slice(snapshot.locks.keys());
        keys.extend_from_slice(plan.locks.keys());
        before.insert(id, (snapshot, policy_snapshot(tx, id).await?));
    }
    let actor = tx
        .account_governance(actor_id)
        .await
        .map_err(|e| AppError::identity(e, request_id))?;
    keys.extend_from_slice(actor.locks.keys());
    keys.extend_from_slice(
        tx.revocation_plan(&IdentityRevocationScope::UserAll { user_id: actor_id })
            .await
            .map_err(|e| AppError::identity(e, request_id))?
            .locks
            .keys(),
    );
    keys.push(IdentityLockKey::Credential {
        issuer: manager.credential().issuer().into(),
        jti: manager.credential().jti().into(),
    });
    tx.acquire_identity_scope(
        &IdentityLockScope::new(keys).map_err(|e| AppError::identity(e, request_id))?,
    )
    .await
    .map_err(|e| AppError::identity(e, request_id))?;
    check_manager(tx, manager, method, route).await?;
    let account = tx
        .identity_account(actor_id)
        .await
        .map_err(|e| AppError::identity(e, request_id))?
        .ok_or(SecurityError::InvalidCredential)?;
    if !account.administrator && !matches!(scope, MutationScope::ClientConsent(_)) {
        return Err(SecurityError::Forbidden.into());
    }
    for (&id, (initial, fingerprint)) in &before {
        let current = tx
            .account_governance(id)
            .await
            .map_err(|e| AppError::identity(e, request_id))?;
        if current.revision != initial.revision
            || current.locks.keys() != initial.locks.keys()
            || policy_snapshot(tx, id).await? != *fingerprint
        {
            return Err(SecurityError::Conflict.into());
        }
    }
    // The same inherited deadline also wraps entity helpers in the callback.
    let output = tx.budget.run(change(&tx.transaction)).await??;
    if !actor.enabled_administrator_ids.is_empty()
        && tx
            .account_governance(actor_id)
            .await
            .map_err(|e| AppError::identity(e, request_id))?
            .enabled_administrator_ids
            .is_empty()
    {
        return Err(AppError::identity(
            IdentityError::LastAdministrator,
            request_id,
        ));
    }
    let mut revoked = Vec::new();
    for (&id, (initial, fingerprint)) in &before {
        let current = tx
            .account_governance(id)
            .await
            .map_err(|e| AppError::identity(e, request_id))?;
        let mut proposed = current.account.clone();
        proposed.auth_epoch = initial.account.auth_epoch;
        validate_account_change(
            &initial.account,
            &proposed,
            initial.enabled_administrator_ids.len(),
            &initial.active_owned_modes,
        )
        .map_err(|e| AppError::identity(e, request_id))?;
        if policy_snapshot(tx, id).await? != *fingerprint {
            tx.identity_execute("UPDATE \"user\" SET auth_epoch=auth_epoch+1,identity_revision=identity_revision+1 WHERE id=$1",vec![id.into()]).await.map_err(|e|AppError::identity(e,request_id))?;
            tx.apply(IdentityWrite::RevokeAccountSessions { user_id: id })
                .await
                .map_err(|e| AppError::identity(e, request_id))?;
            for &environment_id in &initial.affected_environments {
                tx.apply(IdentityWrite::RecomputeGeneration {
                    environment_id,
                    user_id: id,
                })
                .await
                .map_err(|e| AppError::identity(e, request_id))?;
            }
            revoked.extend_from_slice(&initial.live_rp_session_ids);
        }
    }
    let event = Uuid::new_v4();
    tx.append_audit(&AuditEvent {
        id: event,
        request_id,
        actor_id: Some(actor_id),
        action: "legacy.management.change",
        object_kind: "management",
        object_id: format!("{method} {route}"),
        result: AuditResult::Success,
        reason: None,
        change_summary: serde_json::json!({"affected_count":revoked.len()}),
        operation_id: None,
        commit_marker: Some(event),
    })
    .await
    .map_err(|e| AppError::identity(e, request_id))?;
    revoked.sort();
    revoked.dedup();
    tx.enqueue_logout(&LogoutEvent {
        audit_event_id: event,
        rp_session_ids: revoked,
    })
    .await
    .map_err(|e| AppError::identity(e, request_id))?;
    manager.ensure_current()?;
    Ok(output)
}
async fn check_manager(
    tx: &mut PgSecurityTransaction,
    manager: &AuthenticatedContext,
    method: &str,
    route: &str,
) -> Result<()> {
    manager.ensure_current()?;
    let row = tx
        .lock_credential(manager.credential())
        .await?
        .ok_or(SecurityError::InvalidCredential)?;
    let (status, key) = status_for_snapshot(&row, now(tx.clock.as_ref())?)?;
    if status.claims != *manager.verified().claims() || key != manager.verified().signing_key_id() {
        return Err(SecurityError::InvalidCredential.into());
    }
    let (policy, operation) = tx
        .uri_policy(manager.verified().claims(), method, route)
        .await?;
    if !matches!(evaluate_policy(&policy, &operation), PolicyEffect::Allow(_)) {
        return Err(SecurityError::Forbidden.into());
    }
    Ok(())
}
async fn policy_snapshot(tx: &PgSecurityTransaction, id: i32) -> Result<serde_json::Value> {
    let row=tx.identity_one("SELECT jsonb_build_object('role',u.role_id,'enabled',u.enabled,'password',u.password,'authorities',COALESCE((SELECT jsonb_agg(jsonb_build_array(a.id,a.method,a.authority_uri) ORDER BY a.id) FROM authority a WHERE COALESCE((SELECT ua.allow FROM user_authority ua WHERE ua.user_id=u.id AND ua.authority_id=a.id),EXISTS(SELECT 1 FROM authority_role ar WHERE ar.role_id=u.role_id AND ar.authority_id=a.id) OR EXISTS(SELECT 1 FROM role r WHERE r.id=u.role_id AND r.name=$2))),'[]'::jsonb),'permissions',COALESCE((SELECT jsonb_agg(jsonb_build_array(p.id,p.application_id,p.name,p.action,p.resource_type,p.resource_pattern) ORDER BY p.id) FROM permission p WHERE COALESCE((SELECT up.allow FROM user_permission up WHERE up.user_id=u.id AND up.permission_id=p.id),EXISTS(SELECT 1 FROM role_permission rp WHERE rp.role_id=u.role_id AND rp.permission_id=p.id))),'[]'::jsonb)) AS fingerprint FROM \"user\" u WHERE u.id=$1",vec![id.into(),crate::entity::role::RoleEnum::Superadmin.get_name().into()]).await.map_err(|e|AppError::identity(e,Uuid::new_v4()))?;
    get(&row, "fingerprint").map_err(|e| AppError::identity(e, Uuid::new_v4()))
}

#[derive(Clone)]
pub struct LegacyRequest {
    id: Uuid,
    budget: SecurityBudget,
    manager: AuthenticatedContext,
}

pub(crate) async fn request_context(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let id = Uuid::new_v4();
    let mut headers = request.headers().clone();
    if matches!(*request.method(), http::Method::GET | http::Method::HEAD) {
        headers.remove(http::header::COOKIE);
    }
    let result = async {
        let budget = SecurityBudget::new(state.security_clock()?)?;
        let manager = browser::authenticate_manager(&state, &headers, &budget).await?;
        Ok::<_, AppError>(LegacyRequest {
            id,
            budget,
            manager,
        })
    }
    .await;
    match result {
        Ok(context) => {
            request.extensions_mut().insert(context);
            next.run(request).await
        }
        Err(error) => error.into_response(),
    }
}

pub(crate) async fn require_directory_administrator(
    state: &AppState,
    context: &LegacyRequest,
) -> Result<()> {
    let manager = &context.manager;
    manager.ensure_current()?;
    let repo =
        PgIdentityRepository::new(state.database.connection.clone(), state.security_clock()?);
    let tx = repo.security.begin_shared(&context.budget).await?;
    let id = manager
        .principal()
        .subject()
        .parse::<i32>()
        .map_err(|_| SecurityError::InvalidCredential)?;
    let allowed = tx
        .identity_account(id)
        .await
        .map_err(|e| AppError::identity(e, context.id))?
        .is_some_and(|a| a.enabled && a.administrator);
    Box::new(tx).rollback().await?;
    if allowed {
        Ok(())
    } else {
        Err(SecurityError::Forbidden.into())
    }
}
