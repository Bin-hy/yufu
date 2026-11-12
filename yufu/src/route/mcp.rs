//! Personal MCP management authenticates an interactive IdP session locally.
use crate::entity::mcp_token::McpTokenVO;
use crate::service::{
    mcp_token,
    security::{
        browser,
        mcp::{McpIssueRequest, McpRotationRequest, McpTokenConfig, McpTokenSet},
    },
};
use crate::{AppState, IdType, result::Result};
use auth::security::budget::SecurityBudget;
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use http::{HeaderMap, HeaderValue, header::CACHE_CONTROL};
use serde::{Deserialize, Serialize};

pub fn get_management_routes() -> Router<AppState> {
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
pub fn get_service_routes() -> Router<AppState> {
    Router::new().route("/oidc/mcp-tokens/introspect", post(introspect_mcp_token))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IssueReq {
    #[serde(default)]
    aud: Option<String>,
    #[serde(default)]
    ttl_days: Option<u32>,
    #[serde(default)]
    name: Option<String>,
}
#[derive(Serialize)]
struct IssueResp {
    id: IdType,
    name: String,
    access_token: String,
    token_type: &'static str,
    expires_in: u64,
    expires_at: String,
    aud: String,
}
fn config(state: &AppState) -> McpTokenConfig {
    McpTokenConfig {
        issuer: state.issuer(),
        max_ttl_days: state.mcp_token_max_ttl_days(),
    }
}
fn no_store() -> HeaderMap {
    HeaderMap::from_iter([(CACHE_CONTROL, HeaderValue::from_static("no-store"))])
}
fn issue_response(set: McpTokenSet) -> IssueResp {
    IssueResp {
        id: set.metadata.id,
        name: set.metadata.name,
        access_token: set.access_token.expose_secret().into(),
        token_type: "Bearer",
        expires_in: set.claims.exp - set.claims.iat,
        expires_at: set
            .metadata
            .expires_at
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
        aud: set.claims.aud,
    }
}
async fn issue_mcp_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<IssueReq>,
) -> Result<(HeaderMap, Json<IssueResp>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let manager = browser::authenticate_manager(&state, &headers, &budget).await?;
    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("未命名令牌");
    let set = browser::lifecycle(&state)?
        .issue_mcp(
            &manager,
            McpIssueRequest {
                application: req
                    .aud
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| crate::error::AppError::BadRequest("aud is required".into()))?,
                name,
                ttl_days: req.ttl_days,
            },
            &config(&state),
            state.id_token_signer().as_ref(),
            &budget,
        )
        .await?;
    Ok((no_store(), Json(issue_response(set))))
}
async fn list_mcp_tokens(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<Vec<McpTokenVO>>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let manager = browser::authenticate_manager(&state, &headers, &budget).await?;
    Ok((
        no_store(),
        Json(
            browser::lifecycle(&state)?
                .list_mcp_tokens(&manager, &budget)
                .await?,
        ),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameReq {
    name: String,
}
async fn rename_mcp_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IdType>,
    Json(req): Json<RenameReq>,
) -> Result<(HeaderMap, Json<String>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let manager = browser::authenticate_manager(&state, &headers, &budget).await?;
    browser::lifecycle(&state)?
        .rename_mcp_token(&manager, id, req.name.trim(), &budget)
        .await?;
    Ok((no_store(), Json("重命名成功".into())))
}
async fn delete_mcp_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IdType>,
) -> Result<(HeaderMap, Json<String>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let manager = browser::authenticate_manager(&state, &headers, &budget).await?;
    browser::lifecycle(&state)?
        .delete_mcp_token(&manager, id, &budget)
        .await?;
    Ok((no_store(), Json("删除成功".into())))
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RefreshReq {
    #[serde(default)]
    ttl_days: Option<u32>,
}
async fn refresh_mcp_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<IdType>,
    Json(req): Json<RefreshReq>,
) -> Result<(HeaderMap, Json<IssueResp>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let manager = browser::authenticate_manager(&state, &headers, &budget).await?;
    let set = browser::lifecycle(&state)?
        .rotate_mcp(
            &manager,
            McpRotationRequest {
                id,
                ttl_days: req.ttl_days,
            },
            &config(&state),
            state.id_token_signer().as_ref(),
            &budget,
        )
        .await?;
    Ok((no_store(), Json(issue_response(set))))
}

#[derive(Deserialize)]
struct IntrospectReq {
    jti: String,
}

#[derive(Serialize)]
struct IntrospectResp {
    revoked: bool,
}

/// Legacy service adapter. It supplies compatibility denylist data only;
/// current-state authentication and service migration are handled by later tasks.
async fn introspect_mcp_token(
    State(state): State<AppState>,
    Json(req): Json<IntrospectReq>,
) -> Result<Json<IntrospectResp>> {
    let revoked = mcp_token::is_revoked(&state.database.connection, &req.jti).await?;
    Ok(Json(IntrospectResp { revoked }))
}
