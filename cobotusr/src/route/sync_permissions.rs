use crate::entity::permission;
use crate::{AppState, IdType};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use sea_orm::{ActiveModelTrait, ActiveValue, Set};
use serde::{Deserialize, Serialize};

/// POST /api/v1/application/sync-permissions — public endpoint with client auth.
///
/// Used by registered applications at startup to declare their required
/// permissions. The client authenticates via client_id + client_secret (same
/// auth as the OIDC token endpoint). Permissions are upserted idempotently.
pub fn get_public_routes() -> Router<AppState> {
    Router::new().route("/application/sync-permissions", post(sync_permissions))
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncReq {
    client_id: String,
    client_secret: String,
    #[serde(default)]
    permissions: Vec<PermEntry>,
    /// Which application the permissions belong to. Defaults to the
    /// authenticating client's own `client_id` (legacy behaviour).
    ///
    /// A service whose SPA client is public (PKCE, no secret) — and can
    /// therefore never authenticate here — may authenticate with a dedicated
    /// confidential machine client and declare `app_key = "<spa client_id>"`,
    /// so its permissions keep living under the application its PDP/`require_perm!`
    /// checks. The target application must already exist (e.g. via
    /// `[[oidc.bootstrap_clients]]`); it is never created implicitly.
    #[serde(default)]
    app_key: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PermEntry {
    name: String,
    action: String,
    #[serde(default)]
    resource_type: Option<String>,
    #[serde(default)]
    resource_pattern: Option<String>,
}

#[derive(Serialize)]
struct SyncResp {
    application_id: IdType,
    synced: usize,
}

async fn sync_permissions(
    State(state): State<AppState>,
    Json(req): Json<SyncReq>,
) -> crate::result::Result<Json<SyncResp>> {
    use auth::security::{budget::SecurityBudget, identity::SecretToken};
    if req.permissions.len() > 100
        || req.permissions.iter().any(|p| {
            p.name.trim().is_empty()
                || p.name.len() > 255
                || p.action.trim().is_empty()
                || p.action.len() > 128
                || p.resource_type.as_ref().is_some_and(|v| v.len() > 255)
                || p.resource_pattern.as_ref().is_some_and(|v| v.len() > 2048)
        })
    {
        return Err(crate::error::AppError::BadRequest(
            "invalid permission batch".into(),
        ));
    }
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let secret = SecretToken::new(req.client_secret.clone())?;
    let app_key = req.app_key.as_deref().unwrap_or(&req.client_id);
    crate::service::identity::legacy_client::execute(
        &state,
        &req.client_id,
        &secret,
        Some(app_key),
        "",
        uuid::Uuid::new_v4(),
        &budget,
        |db, _, app| {
            let permissions = req.permissions.clone();
            let application_id = app.map(|app| app.id);
            Box::pin(async move {
                let application_id = application_id.ok_or(crate::error::AppError::BadRequest(
                    "missing explicit application".into(),
                ))?;
                let mut synced = 0usize;
                let now = chrono::Utc::now().naive_utc();
                for entry in &permissions {
                    if permission::Entity::get_by_app_and_name(db, application_id, &entry.name)
                        .await?
                        .is_some()
                    {
                        continue; // already exists
                    }
                    permission::ActiveModel {
                        id: ActiveValue::NotSet,
                        application_id: Set(application_id),
                        name: Set(entry.name.clone()),
                        description: Set(None),
                        action: Set(entry.action.clone()),
                        resource_type: Set(entry.resource_type.clone()),
                        resource_pattern: Set(entry
                            .resource_pattern
                            .clone()
                            .or_else(|| Some("*".to_string()))),
                        create_time: Set(now),
                        update_time: Set(now),
                    }
                    .insert(db)
                    .await?;
                    synced += 1;
                }

                Ok(Json(SyncResp {
                    application_id,
                    synced,
                }))
            })
        },
    )
    .await
}
