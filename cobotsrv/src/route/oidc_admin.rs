use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::result::Result;
use crate::service::oidc::OidcService;

// --- Request / Response types ---

#[derive(Deserialize)]
pub struct ClientCreateReq {
    pub name: String,
    pub homepage_url: Option<String>,
    pub description: Option<String>,
    pub redirect_uri: String,
}

#[derive(Serialize)]
pub struct ClientVO {
    pub id: i32,
    pub client_id: String,
    pub name: String,
    pub redirect_uris: String,
    pub allowed_scopes: String,
    pub grant_types: String,
    pub create_time: String,
    pub update_time: String,
}

impl From<crate::entity::oauth_client::Model> for ClientVO {
    fn from(m: crate::entity::oauth_client::Model) -> Self {
        Self {
            id: m.id,
            client_id: m.client_id,
            name: m.name,
            redirect_uris: m.redirect_uris,
            allowed_scopes: m.allowed_scopes,
            grant_types: m.grant_types,
            create_time: m.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            update_time: m.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct ClientCreateResp {
    #[serde(flatten)]
    pub client: ClientVO,
    pub client_secret: String, // returned only once
}

#[derive(Serialize)]
pub struct SecretResp {
    pub client_secret: String,
}

#[derive(Deserialize)]
pub struct ClientUpdateReq {
    pub name: Option<String>,
    pub redirect_uris: Option<Vec<String>>,
    pub allowed_scopes: Option<Vec<String>>,
    pub grant_types: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
pub struct ScopeInsertReq {
    pub name: String,
    pub description: Option<String>,
}

// --- Routes ---

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/clients", get(list_clients).post(create_client))
        .route(
            "/oidc/client/{id}",
            get(get_client).patch(update_client).delete(delete_client),
        )
        .route("/oidc/client/{id}/secret", post(regenerate_secret))
        .route("/oidc/scope", post(add_scope).get(list_scopes))
}

// --- Client Handlers ---

async fn list_clients(State(state): State<AppState>) -> Result<Json<Vec<ClientVO>>> {
    let db = &state.database.connection;
    let list = OidcService::list_clients(db).await?;
    Ok(Json(list.into_iter().map(ClientVO::from).collect()))
}

async fn create_client(
    State(state): State<AppState>,
    Json(req): Json<ClientCreateReq>,
) -> Result<Json<ClientCreateResp>> {
    let db = &state.database.connection;
    let (inserted, client_secret) = OidcService::register_client(
        db,
        uuid::Uuid::new_v4().to_string(), // auto-generate client_id
        req.name,
        vec![req.redirect_uri],
        vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
        ],
        vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
    )
    .await?;
    Ok(Json(ClientCreateResp {
        client: ClientVO::from(inserted),
        client_secret,
    }))
}

async fn get_client(State(state): State<AppState>, Path(id): Path<i32>) -> Result<Json<ClientVO>> {
    let db = &state.database.connection;
    let client = OidcService::get_client_by_id(db, id).await?;
    Ok(Json(ClientVO::from(client)))
}

async fn update_client(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<ClientUpdateReq>,
) -> Result<Json<ClientVO>> {
    let db = &state.database.connection;
    let updated = OidcService::update_client(
        db,
        id,
        req.name,
        req.redirect_uris,
        req.allowed_scopes,
        req.grant_types,
    )
    .await?;
    Ok(Json(ClientVO::from(updated)))
}

async fn delete_client(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<http::StatusCode> {
    let db = &state.database.connection;
    OidcService::delete_client(db, id).await?;
    Ok(http::StatusCode::NO_CONTENT)
}

async fn regenerate_secret(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<SecretResp>> {
    let db = &state.database.connection;
    let (_updated, new_secret) = OidcService::regenerate_client_secret(db, id).await?;
    Ok(Json(SecretResp {
        client_secret: new_secret,
    }))
}

// --- Scope Handlers ---

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
