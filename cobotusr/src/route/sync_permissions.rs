use crate::entity::{application, permission};
use crate::service::oidc::OidcService;
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

#[derive(Deserialize)]
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

#[derive(Deserialize)]
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
    let db = &state.database.connection;

    // Authenticate client
    let client =
        OidcService::authenticate_client(db, Some(&req.client_id), Some(&req.client_secret))
            .await?;

    // Permissions belong to `app_key`, which may differ from the
    // authenticating client (machine-client proxy pattern). When unset,
    // fall back to the client's own client_id (legacy behaviour).
    let app_key = req.app_key.as_deref().unwrap_or(&client.client_id);

    // Find application row for the app_key. Explicit app_keys must already
    // exist (bootstrap-created); only the legacy implicit path creates one.
    let app = match application::Entity::get_by_app_key(db, app_key).await? {
        Some(a) => a,
        None => {
            if req.app_key.is_some() {
                return Err(crate::error::AppError::BadRequest(format!(
                    "application not found for app_key '{app_key}'; \
                     register it via [[oidc.bootstrap_clients]] first"
                )));
            }
            let now = chrono::Utc::now().naive_utc();
            application::ActiveModel {
                id: ActiveValue::NotSet,
                app_key: Set(client.client_id.clone()),
                name: Set(client.name.clone()),
                description: Set(None),
                oidc_client_id: Set(Some(client.id)),
                create_time: Set(now),
                update_time: Set(now),
            }
            .insert(db)
            .await?
        }
    };

    // Upsert permissions
    let mut synced = 0usize;
    let now = chrono::Utc::now().naive_utc();
    for entry in &req.permissions {
        if permission::Entity::get_by_app_and_name(db, app.id, &entry.name)
            .await?
            .is_some()
        {
            continue; // already exists
        }
        permission::ActiveModel {
            id: ActiveValue::NotSet,
            application_id: Set(app.id),
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

    tracing::info!(
        client_id = %client.client_id,
        app_key = %app_key,
        proxy = req.app_key.is_some(),
        synced,
        "permissions synced for application"
    );
    Ok(Json(SyncResp {
        application_id: app.id,
        synced,
    }))
}
