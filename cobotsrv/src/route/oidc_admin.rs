use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
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
) -> Result<Json<oauth_client::Model>> {
    let db = &state.database.connection;
    let now = Utc::now().naive_utc();
    let am = oauth_client::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        client_id: Set(req.client_id),
        name: Set(req.name),
        redirect_uris: Set(req.redirect_uris.join(",")),
        allowed_scopes: Set(req.allowed_scopes.join(",")),
        create_time: Set(now),
        update_time: Set(now),
    };
    let inserted = am.insert(db).await.map_err(AppError::from)?;
    Ok(Json(inserted))
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
