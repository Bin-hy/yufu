use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::result::Result;
use crate::service::oidc::OidcService;

#[derive(Deserialize, Serialize)]
pub struct ClientInsertReq {
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub allowed_scopes: Vec<String>,
    #[serde(default = "default_grant_types")]
    pub grant_types: Vec<String>,
}

fn default_grant_types() -> Vec<String> {
    vec![
        "authorization_code".to_string(),
        "refresh_token".to_string(),
    ]
}

#[derive(Serialize)]
pub struct ClientInsertResp {
    pub id: i32,
    pub client_id: String,
    pub client_secret: String, // returned only once
    pub name: String,
    pub redirect_uris: String,
    pub allowed_scopes: String,
    pub grant_types: String,
}

#[derive(Deserialize, Serialize)]
pub struct ScopeInsertReq {
    pub name: String,
    pub description: Option<String>,
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/client", post(add_client))
        .route("/oidc/scope", post(add_scope).get(list_scopes))
}

async fn add_client(
    State(state): State<AppState>,
    Json(req): Json<ClientInsertReq>,
) -> Result<Json<ClientInsertResp>> {
    let db = &state.database.connection;
    let (inserted, client_secret) = OidcService::register_client(
        db,
        req.client_id,
        req.name,
        req.redirect_uris,
        req.allowed_scopes,
        req.grant_types,
    )
    .await?;

    Ok(Json(ClientInsertResp {
        id: inserted.id,
        client_id: inserted.client_id,
        client_secret, // only returned once
        name: inserted.name,
        redirect_uris: inserted.redirect_uris,
        allowed_scopes: inserted.allowed_scopes,
        grant_types: inserted.grant_types,
    }))
}

async fn add_scope(
    State(state): State<AppState>,
    Json(req): Json<ScopeInsertReq>,
) -> Result<Json<crate::entity::oauth_scope::Model>> {
    let db = &state.database.connection;
    let inserted = OidcService::add_scope(db, req.name, req.description).await?;
    Ok(Json(inserted))
}

async fn list_scopes(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::entity::oauth_scope::Model>>> {
    let db = &state.database.connection;
    let list = OidcService::list_scopes(db).await?;
    Ok(Json(list))
}
