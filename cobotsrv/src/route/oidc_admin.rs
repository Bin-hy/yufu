use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use rand::RngCore;
use rand::rngs::OsRng;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::entity::{oauth_client, oauth_scope};
use crate::error::AppError;
use crate::result::Result;

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
    let now = Utc::now().naive_utc();

    // Generate a random client_secret
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    let client_secret = hex::encode(buf);
    let secret_hash = bcrypt::hash(&client_secret, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("hash error: {}", e)))?;

    let am = oauth_client::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        client_id: Set(req.client_id),
        name: Set(req.name),
        redirect_uris: Set(req.redirect_uris.join(",")),
        allowed_scopes: Set(req.allowed_scopes.join(",")),
        client_secret_hash: Set(secret_hash),
        grant_types: Set(req.grant_types.join(",")),
        create_time: Set(now),
        update_time: Set(now),
    };
    let inserted = am.insert(db).await.map_err(AppError::from)?;
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
) -> Result<Json<oauth_scope::Model>> {
    let db = &state.database.connection;
    let now = Utc::now().naive_utc();
    let am = oauth_scope::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        name: Set(req.name),
        description: Set(req.description),
        create_time: Set(now),
        update_time: Set(now),
    };
    let inserted = am.insert(db).await.map_err(AppError::from)?;
    Ok(Json(inserted))
}

async fn list_scopes(State(state): State<AppState>) -> Result<Json<Vec<oauth_scope::Model>>> {
    let db = &state.database.connection;
    let list = oauth_scope::Entity::find()
        .all(db)
        .await
        .map_err(AppError::from)?;
    Ok(Json(list))
}
