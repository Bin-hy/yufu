//! MCP access-token lifecycle: issue, list, rename, refresh (rotate), revoke, introspect.
//!
//! `POST /api/v1/oidc/mcp-tokens` mints a **long-lived** access token so operators can paste
//! it into an MCP client (Claude Code / Desktop) without re-running the 1-hour login flow.
//! Unlike the original mint-and-forget design, each issued token is now **persisted and
//! revocable**: one `mcp_token` row per token, keyed by the token's `jti`. cobotsrv checks
//! that `jti` against this denylist on every MCP request (see `cobotsrv/src/mcp/revocation.rs`),
//! so `DELETE` and `refresh` actually invalidate a token within the resource server's cache TTL.
//!
//! All routes operate on the **caller's own** tokens. Only issue and refresh verify the
//! caller still holds `mcp:access` for the target audience (the seed migration
//! `m20260731_000001_seed_mcp_access` grants it to the default `User` role for cobotsrv;
//! superadmin holds it implicitly); list/rename/delete are purely ownership-scoped.
//!
//! The introspection endpoint mirrors the PDP `/enforce` pattern (called by cobotsrv
//! with the incoming bearer), but lives on a JWT-only route group — no user load and no
//! URI-RBAC middleware — so a misconfigured authority row or a deleted user cannot turn
//! its responses into non-JSON errors that cobotsrv's fail-open revocation client would
//! silently treat as "not revoked".
//!
//! Known tradeoff: MCP tokens bake the user's roles/permissions into the JWT at mint time,
//! for up to `oidc.mcp_token_max_ttl_days` (default 90 days). Demoting a user (removing a
//! role or permission) does NOT affect already-minted tokens — they keep their mint-time
//! claims until expiry. The only way to cut one off early is whole-token revocation
//! (`DELETE` or `refresh`), enforced via the `jti` denylist above.

use auth::claims::AccessTokenClaims;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::post,
};
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::IdType;
use crate::entity::mcp_token::McpTokenVO;
use crate::entity::user;
use crate::error::AppError;
use crate::result::Result;
use crate::service::{mcp_token, permission};

const DEFAULT_TTL_DAYS: u32 = 30;
const DEFAULT_TOKEN_NAME: &str = "未命名令牌";
/// Maximum number of active (non-revoked, non-expired) MCP tokens per user.
const MAX_ACTIVE_TOKENS_PER_USER: u64 = 20;

/// Owner-facing token management routes (JWT + user load + URI-RBAC middleware group).
pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/oidc/mcp-tokens",
            post(issue_mcp_token).get(list_mcp_tokens),
        )
        .route(
            "/oidc/mcp-tokens/{id}",
            axum::routing::patch(rename_mcp_token).delete(delete_mcp_token),
        )
        .route("/oidc/mcp-tokens/{id}/refresh", post(refresh_mcp_token))
}

/// Service-to-service routes: JWT-verified only (no user load, no URI-RBAC), so
/// cobotsrv's revocation client always gets either a JSON answer or a plain 401.
pub fn get_service_routes() -> Router<AppState> {
    Router::new().route("/oidc/mcp-tokens/introspect", post(introspect_mcp_token))
}

#[derive(Deserialize)]
struct IssueReq {
    /// Target resource (audience). Defaults to the caller token's `aud`.
    #[serde(default)]
    aud: Option<String>,
    /// Requested lifetime in days. Defaults to 30, clamped to the configured max.
    #[serde(default)]
    ttl_days: Option<u32>,
    /// User-facing label. Defaults to "未命名令牌".
    #[serde(default)]
    name: Option<String>,
}

#[derive(Serialize)]
struct IssueResp {
    id: IdType,
    name: String,
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    expires_at: String,
    aud: String,
}

/// Resolve the caller's permissions for `aud`, require `mcp:access`, mint a token with a
/// fresh `jti`, and persist its `mcp_token` record. Returns (row, token, ttl_seconds).
///
/// Reads (permission resolution) go through the shared connection; the row insert goes
/// through `db`, which is a transaction during refresh so mint+revoke commit atomically.
async fn mint_and_store(
    state: &AppState,
    db: &impl ConnectionTrait,
    user_id: IdType,
    aud: &str,
    name: &str,
    ttl_days: Option<u32>,
) -> Result<(crate::entity::mcp_token::Model, String, i64)> {
    let (roles, perms) =
        permission::resolve_user_permissions(&state.database.connection, user_id, aud).await?;

    // Deliberately a plain name check on the resolved set (superadmin-aware, explicit
    // denies already applied) rather than `permission::enforce`: enforce's action/
    // resource matching adds semantics the mint flow doesn't need.
    if !perms.iter().any(|p| p == "mcp:access") {
        return Err(AppError::Forbidden(format!(
            "user lacks mcp:access for audience '{aud}'"
        )));
    }

    let max_days = state.mcp_token_max_ttl_days().max(1);
    let days = ttl_days.unwrap_or(DEFAULT_TTL_DAYS).clamp(1, max_days);
    let ttl_seconds = days as i64 * 86_400;

    // Single timestamp for the row's created_at/expires_at; the JWT is signed
    // immediately after with the same TTL, so row expiry and `exp` agree.
    let now = chrono::Utc::now().naive_utc();
    let expires_at = now + chrono::Duration::seconds(ttl_seconds);

    let jti = uuid::Uuid::new_v4().to_string();
    let token = state.mint_access_token(user_id, aud, ttl_seconds, roles, perms, &jti)?;
    let row = mcp_token::create(db, user_id, name, &jti, aud, now, expires_at).await?;
    Ok((row, token, ttl_seconds))
}

fn issue_resp(row: crate::entity::mcp_token::Model, token: String, ttl_seconds: i64) -> IssueResp {
    IssueResp {
        id: row.id,
        name: row.name,
        access_token: token,
        token_type: "Bearer",
        expires_in: ttl_seconds,
        expires_at: row.expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        aud: row.aud,
    }
}

/// POST /oidc/mcp-tokens — mint + persist a new MCP token. Token value returned **once**.
/// Limited to [`MAX_ACTIVE_TOKENS_PER_USER`] active tokens per user.
async fn issue_mcp_token(
    State(state): State<AppState>,
    Extension(claims): Extension<AccessTokenClaims>,
    Extension(user): Extension<user::Model>,
    Json(req): Json<IssueReq>,
) -> Result<Json<IssueResp>> {
    let aud = req
        .aud
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| claims.aud.clone());
    let name = req
        .name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TOKEN_NAME.to_string());

    let active = mcp_token::count_active_by_user(&state.database.connection, user.id).await?;
    if active >= MAX_ACTIVE_TOKENS_PER_USER {
        return Err(AppError::BadRequest(format!(
            "active MCP token limit reached ({MAX_ACTIVE_TOKENS_PER_USER}); revoke an existing token first"
        )));
    }

    let (row, token, ttl_seconds) = mint_and_store(
        &state,
        &state.database.connection,
        user.id,
        &aud,
        &name,
        req.ttl_days,
    )
    .await?;
    Ok(Json(issue_resp(row, token, ttl_seconds)))
}

/// GET /oidc/mcp-tokens — the caller's active tokens (metadata only, never the value).
async fn list_mcp_tokens(
    State(state): State<AppState>,
    Extension(user): Extension<user::Model>,
) -> Result<Json<Vec<McpTokenVO>>> {
    let list = mcp_token::list_active_by_user(&state.database.connection, user.id).await?;
    Ok(Json(list))
}

#[derive(Deserialize)]
struct RenameReq {
    name: String,
}

/// PATCH /oidc/mcp-tokens/{id} — rename one of the caller's tokens.
async fn rename_mcp_token(
    State(state): State<AppState>,
    Extension(user): Extension<user::Model>,
    Path(id): Path<IdType>,
    Json(req): Json<RenameReq>,
) -> Result<Json<String>> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".to_string()));
    }
    if !mcp_token::rename(&state.database.connection, user.id, id, name).await? {
        return Err(AppError::not_found("token not found"));
    }
    Ok(Json("重命名成功".to_string()))
}

/// DELETE /oidc/mcp-tokens/{id} — revoke (soft-delete) one of the caller's tokens.
async fn delete_mcp_token(
    State(state): State<AppState>,
    Extension(user): Extension<user::Model>,
    Path(id): Path<IdType>,
) -> Result<Json<String>> {
    if !mcp_token::revoke(&state.database.connection, user.id, id).await? {
        return Err(AppError::not_found("token not found"));
    }
    Ok(Json("删除成功".to_string()))
}

#[derive(Deserialize, Default)]
struct RefreshReq {
    #[serde(default)]
    ttl_days: Option<u32>,
}

/// POST /oidc/mcp-tokens/{id}/refresh — rotate: mint a fresh token (new jti/expiry) and
/// revoke the old one atomically (single transaction, so a failed revoke cannot leave
/// both tokens live). The new token value is returned **once**; the old token stops
/// working once cobotsrv's revocation cache refreshes.
async fn refresh_mcp_token(
    State(state): State<AppState>,
    Extension(user): Extension<user::Model>,
    Path(id): Path<IdType>,
    Json(req): Json<RefreshReq>,
) -> Result<Json<IssueResp>> {
    let Some(old) = mcp_token::find_owned(&state.database.connection, user.id, id).await? else {
        return Err(AppError::not_found("token not found"));
    };
    if old.revoked {
        return Err(AppError::BadRequest("token already revoked".to_string()));
    }

    // Mint + persist the replacement and revoke the old row in one transaction:
    // concurrent refreshes cannot yield two live tokens, and a failed revoke
    // rolls the new row back instead of silently keeping the old token valid.
    let txn = state.database.connection.begin().await?;
    let (row, token, ttl_seconds) =
        mint_and_store(&state, &txn, user.id, &old.aud, &old.name, req.ttl_days).await?;
    mcp_token::revoke(&txn, user.id, old.id).await?;
    txn.commit().await?;

    Ok(Json(issue_resp(row, token, ttl_seconds)))
}

#[derive(Deserialize)]
struct IntrospectReq {
    jti: String,
}

#[derive(Serialize)]
struct IntrospectResp {
    revoked: bool,
}

/// POST /oidc/mcp-tokens/introspect — denylist check for cobotsrv: is this `jti` revoked?
/// Unknown jtis (ordinary OIDC tokens) report `revoked: false`.
///
/// Registered on the JWT-only service route group (see `get_service_routes`): cobotsrv's
/// revocation client fails open on non-JSON error responses, so this endpoint must never
/// depend on user loading or URI-RBAC authority rows.
async fn introspect_mcp_token(
    State(state): State<AppState>,
    Json(req): Json<IntrospectReq>,
) -> Result<Json<IntrospectResp>> {
    let revoked = mcp_token::is_revoked(&state.database.connection, &req.jti).await?;
    Ok(Json(IntrospectResp { revoked }))
}
