use axum::extract::State;
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::result::Result;
use crate::store::AuthCodeEntry;
use crate::{AppState, IdType};
use auth::claims::Claims;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use rand::RngCore;
use rand::rngs::OsRng;
use sea_orm::{ActiveModelTrait, Set};
use sha2::{Digest, Sha256};

#[derive(Serialize, Clone)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: Option<String>,
    pub response_types_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
}

// --- PLACEHOLDER_OIDC_TYPES ---

#[derive(Deserialize)]
pub struct TokenRequestAuthorizationCode {
    pub grant_type: String,
    pub code: String,
    pub code_verifier: String,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub redirect_uri: Option<String>,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub id_token: String,
    pub token_type: String,
    pub expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

#[derive(Serialize)]
pub struct UserInfoResp {
    pub sub: IdType,
    pub preferred_username: String,
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct RefreshTokenRequest {
    pub grant_type: String,
    pub refresh_token: String,
    #[serde(default)]
    pub client_id: Option<String>,
}

#[derive(Deserialize)]
pub struct AuthorizeQuery {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub code_challenge: String,
    #[serde(default = "default_code_challenge_method")]
    pub code_challenge_method: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
}

fn default_code_challenge_method() -> String {
    "S256".to_string()
}

// --- PLACEHOLDER_OIDC_ROUTES ---

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(openid_configuration),
        )
        .route(
            "/oidc/token",
            post(token_authorization_code).patch(token_refresh),
        )
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/userinfo", get(userinfo))
        .route("/oidc/authorize", get(authorize))
}

fn get_issuer_base(state: &AppState) -> String {
    if state.config.http.public.is_empty() {
        format!("http://{}", state.config.http.listen)
    } else {
        state.config.http.public.clone()
    }
}

async fn openid_configuration(State(state): State<AppState>) -> Result<Json<OidcDiscovery>> {
    let base = get_issuer_base(&state);
    let resp = OidcDiscovery {
        issuer: base.clone(),
        authorization_endpoint: format!("{}/api/v1/oidc/authorize", base),
        token_endpoint: format!("{}/oidc/token", base),
        userinfo_endpoint: format!("{}/api/v1/oidc/userinfo", base),
        jwks_uri: None,
        response_types_supported: vec!["code".to_string()],
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: vec!["HS256".to_string()],
    };
    Ok(Json(resp))
}

// --- PLACEHOLDER_OIDC_HANDLERS ---

async fn token_authorization_code(
    State(state): State<AppState>,
    Json(req): Json<TokenRequestAuthorizationCode>,
) -> Result<Json<TokenResponse>> {
    if req.grant_type != "authorization_code" {
        return Err(crate::error::AppError::BadRequest(
            "unsupported grant_type, expected authorization_code".to_string(),
        ));
    }

    let entry =
        state
            .storage
            .take_auth_code(&req.code)
            .ok_or(crate::error::AppError::BadRequest(
                "authorization code invalid or already used".to_string(),
            ))?;
    if Utc::now().naive_utc() > entry.expires_at {
        return Err(crate::error::AppError::BadRequest(
            "authorization code expired".to_string(),
        ));
    }

    // Validate client_id and redirect_uri consistency
    if entry.client_id != req.client_id || entry.redirect_uri != req.redirect_uri {
        return Err(crate::error::AppError::BadRequest(
            "client_id or redirect_uri mismatch".to_string(),
        ));
    }

    // Verify PKCE
    let ok = match entry.code_challenge_method.as_str() {
        "plain" => entry.code_challenge == req.code_verifier,
        "S256" => {
            let digest = Sha256::digest(req.code_verifier.as_bytes());
            let challenge = URL_SAFE_NO_PAD.encode(digest);
            entry.code_challenge == challenge
        }
        _ => false,
    };
    if !ok {
        return Err(crate::error::AppError::BadRequest(
            "code_verifier validation failed".to_string(),
        ));
    }

    let base = get_issuer_base(&state);
    let access_token = state
        .auth_state
        .token_manager
        .generate_token::<IdType>(entry.user_id)?;

    let id_token = state.auth_state.token_manager.generate_id_token::<IdType>(
        entry.user_id,
        &base,
        entry.client_id.as_deref(),
        entry.nonce.as_deref(),
        Some(&access_token),
    )?;

    // PLACEHOLDER_TOKEN_PERSIST_1 ---
    // Create refresh_token and persist token record
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    let refresh_token = hex::encode(buf);
    let now = Utc::now().naive_utc();
    let access_expires = now + Duration::seconds(state.config.auth.exp_seconds);
    let refresh_expires = now + Duration::seconds(state.config.auth.refresh_exp_seconds);
    let new_rec = crate::entity::oauth_token::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        user_id: Set(entry.user_id),
        client_id: Set(entry.client_id.clone().unwrap_or_default()),
        access_token: Set(access_token.clone()),
        id_token: Set(id_token.clone()),
        refresh_token: Set(refresh_token.clone()),
        scope: Set(entry.scope.clone()),
        token_type: Set("Bearer".to_string()),
        create_time: Set(now),
        access_expires_at: Set(access_expires),
        refresh_expires_at: Set(refresh_expires),
        update_time: Set(now),
    };
    new_rec.insert(&state.database.connection).await?;

    Ok(Json(TokenResponse {
        access_token,
        id_token,
        token_type: "Bearer".to_string(),
        expires_in: state.config.auth.exp_seconds,
        refresh_token: Some(refresh_token),
    }))
}

// --- PLACEHOLDER_OIDC_REFRESH ---

async fn token_refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshTokenRequest>,
) -> Result<Json<TokenResponse>> {
    if req.grant_type != "refresh_token" {
        return Err(crate::error::AppError::BadRequest(
            "unsupported grant_type, expected refresh_token".to_string(),
        ));
    }

    let db = &state.database.connection;
    let rec = crate::entity::oauth_token::Entity::find_by_refresh_token(db, &req.refresh_token)
        .await?
        .ok_or(crate::error::AppError::Unauthorized(
            "invalid refresh_token".to_string(),
        ))?;

    if Utc::now().naive_utc() > rec.refresh_expires_at {
        return Err(crate::error::AppError::Unauthorized(
            "refresh_token expired".to_string(),
        ));
    }

    let base = get_issuer_base(&state);
    let access_token = state
        .auth_state
        .token_manager
        .generate_token::<IdType>(rec.user_id)?;

    let id_token = state.auth_state.token_manager.generate_id_token::<IdType>(
        rec.user_id,
        &base,
        Some(&rec.client_id),
        None,
        Some(&access_token),
    )?;

    Ok(Json(TokenResponse {
        access_token,
        id_token,
        token_type: "Bearer".to_string(),
        expires_in: state.config.auth.exp_seconds,
        refresh_token: None,
    }))
}

async fn userinfo(
    claims: Claims<IdType>,
    State(state): State<AppState>,
) -> Result<Json<UserInfoResp>> {
    let db = &state.database.connection;
    let user_vo = crate::service::user::UserService::get_user_by_id(db, claims.id).await?;
    Ok(Json(UserInfoResp {
        sub: user_vo.id,
        preferred_username: user_vo.username,
        name: user_vo.nickname,
    }))
}

async fn authorize(
    claims: Claims<IdType>,
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<AuthorizeQuery>,
) -> Result<Redirect> {
    if q.response_type != "code" {
        return Err(crate::error::AppError::BadRequest(
            "unsupported response_type, expected code".to_string(),
        ));
    }

    let method = q.code_challenge_method.clone();
    if method != "S256" && method != "plain" {
        return Err(crate::error::AppError::BadRequest(
            "unsupported code_challenge_method".to_string(),
        ));
    }

    let db = &state.database.connection;
    let client_id = q.client_id.clone();
    let redirect_uri = q.redirect_uri.clone();

    let client = crate::entity::oauth_client::Entity::get_by_client_id(db, &client_id)
        .await?
        .ok_or(crate::error::AppError::Unauthorized(
            "invalid client_id".to_string(),
        ))?;
    if !client.has_redirect(&redirect_uri) {
        return Err(crate::error::AppError::Unauthorized(
            "invalid redirect_uri".to_string(),
        ));
    }

    let req_scopes: Vec<String> = q
        .scope
        .clone()
        .unwrap_or_else(|| "openid".to_string())
        .split(' ')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !client.allows_scopes(&req_scopes) {
        return Err(crate::error::AppError::Unauthorized(
            "requested scope not allowed by client".to_string(),
        ));
    }
    if !crate::entity::oauth_scope::Entity::exists_by_names(db, &req_scopes).await? {
        return Err(crate::error::AppError::Unauthorized(
            "requested scope does not exist".to_string(),
        ));
    }

    // Generate authorization code
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    let code = hex::encode(buf);

    // Store code (5 min TTL)
    let now = Utc::now().naive_utc();
    let entry = AuthCodeEntry {
        user_id: claims.id,
        client_id: Some(client_id.clone()),
        redirect_uri: Some(redirect_uri.clone()),
        code_challenge: q.code_challenge.clone(),
        code_challenge_method: method,
        scope: q.scope.clone(),
        state: q.state.clone(),
        nonce: q.nonce.clone(),
        created_at: now,
        expires_at: now + Duration::minutes(5),
    };
    state.storage.insert_auth_code(code.clone(), entry);

    // Redirect back to client
    let mut url = redirect_uri;
    let sep = if url.contains('?') { '&' } else { '?' };
    url.push(sep);
    url.push_str(&format!("code={}", code));
    if let Some(st) = q.state {
        url.push_str(&format!("&state={}", st));
    }
    Ok(Redirect::to(&url))
}
