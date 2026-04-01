use axum::extract::State;
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::result::Result;
use crate::service::oidc::{CreateAuthCodeParams, OidcService, get_issuer_base};
use crate::{AppState, IdType};
use auth::claims::Claims;
use chrono::Utc;

#[derive(Serialize, Clone)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: Option<String>,
    pub revocation_endpoint: String,
    pub introspection_endpoint: String,
    pub response_types_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub scopes_supported: Vec<String>,
    pub claims_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
}

/// Unified token request — fields are optional depending on grant_type
#[derive(Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    // authorization_code fields
    pub code: Option<String>,
    pub code_verifier: Option<String>,
    pub redirect_uri: Option<String>,
    // refresh_token fields
    pub refresh_token: Option<String>,
    // client auth (also via Basic header)
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    // client_credentials scope
    pub scope: Option<String>,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    pub token_type: String,
    pub expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

#[derive(Serialize)]
pub struct UserInfoResp {
    pub sub: IdType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
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

#[derive(Deserialize)]
pub struct RevokeRequest {
    pub token: String,
    #[serde(default = "default_token_type_hint")]
    pub token_type_hint: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

fn default_token_type_hint() -> String {
    "refresh_token".to_string()
}

#[derive(Deserialize)]
pub struct IntrospectRequest {
    pub token: String,
    #[serde(default)]
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Serialize)]
pub struct IntrospectResponse {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
}

// --- Routes ---

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(openid_configuration),
        )
        .route("/.well-known/jwks.json", get(jwks))
        .route("/oidc/token", post(token_endpoint))
        .route("/oidc/revoke", post(revoke))
        .route("/oidc/introspect", post(introspect))
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/userinfo", get(userinfo))
        .route("/oidc/authorize", get(authorize))
}

// --- Handlers ---

async fn openid_configuration(State(state): State<AppState>) -> Result<Json<OidcDiscovery>> {
    let base = get_issuer_base(&state);
    let resp = OidcDiscovery {
        issuer: base.clone(),
        authorization_endpoint: format!("{}/api/v1/oidc/authorize", base),
        token_endpoint: format!("{}/oidc/token", base),
        userinfo_endpoint: format!("{}/api/v1/oidc/userinfo", base),
        jwks_uri: Some(format!("{}/.well-known/jwks.json", base)),
        revocation_endpoint: format!("{}/oidc/revoke", base),
        introspection_endpoint: format!("{}/oidc/introspect", base),
        response_types_supported: vec!["code".to_string()],
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: vec![state.id_token_signer.algorithm().to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
            "client_credentials".to_string(),
        ],
        token_endpoint_auth_methods_supported: vec!["client_secret_post".to_string()],
        scopes_supported: vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
            "phone".to_string(),
            "address".to_string(),
            "offline_access".to_string(),
        ],
        claims_supported: vec![
            "sub".to_string(),
            "iss".to_string(),
            "aud".to_string(),
            "exp".to_string(),
            "iat".to_string(),
            "nonce".to_string(),
            "at_hash".to_string(),
            "preferred_username".to_string(),
            "name".to_string(),
            "nickname".to_string(),
            "email".to_string(),
        ],
        code_challenge_methods_supported: vec!["S256".to_string(), "plain".to_string()],
    };
    Ok(Json(resp))
}

async fn jwks(State(state): State<AppState>) -> axum::response::Response {
    use axum::response::IntoResponse;
    match state.id_token_signer.jwks_json() {
        Some(json) => ([(http::header::CONTENT_TYPE, "application/json")], json).into_response(),
        None => (
            http::StatusCode::NOT_FOUND,
            "JWKS not available (HMAC mode)",
        )
            .into_response(),
    }
}

/// Unified token endpoint — dispatches by grant_type
async fn token_endpoint(
    State(state): State<AppState>,
    Json(req): Json<TokenRequest>,
) -> Result<Json<TokenResponse>> {
    match req.grant_type.as_str() {
        "authorization_code" => handle_authorization_code(&state, &req).await,
        "refresh_token" => handle_refresh_token(&state, &req).await,
        "client_credentials" => handle_client_credentials(&state, &req).await,
        _ => Err(crate::error::AppError::BadRequest(format!(
            "unsupported grant_type: {}",
            req.grant_type
        ))),
    }
}

async fn handle_authorization_code(
    state: &AppState,
    req: &TokenRequest,
) -> Result<Json<TokenResponse>> {
    let code = req
        .code
        .as_deref()
        .ok_or(crate::error::AppError::BadRequest(
            "missing code".to_string(),
        ))?;
    let code_verifier = req
        .code_verifier
        .as_deref()
        .ok_or(crate::error::AppError::BadRequest(
            "missing code_verifier".to_string(),
        ))?;

    let db = &state.database.connection;
    let entry = OidcService::take_authorization_code(db, code)
        .await?
        .ok_or(crate::error::AppError::BadRequest(
            "authorization code invalid or already used".to_string(),
        ))?;
    if Utc::now().naive_utc() > entry.expires_at {
        return Err(crate::error::AppError::BadRequest(
            "authorization code expired".to_string(),
        ));
    }

    // Authenticate client
    let client = OidcService::authenticate_client(
        db,
        req.client_id.as_deref(),
        req.client_secret.as_deref(),
    )
    .await?;

    // Validate consistency
    if entry.client_id != client.client_id
        || entry.redirect_uri.as_deref() != req.redirect_uri.as_deref()
    {
        return Err(crate::error::AppError::BadRequest(
            "client_id or redirect_uri mismatch".to_string(),
        ));
    }
    if !OidcService::client_allows_grant_type(&client, "authorization_code") {
        return Err(crate::error::AppError::BadRequest(
            "client not allowed authorization_code grant".to_string(),
        ));
    }

    // Verify PKCE
    if !OidcService::verify_pkce(
        &entry.code_challenge,
        &entry.code_challenge_method,
        code_verifier,
    ) {
        return Err(crate::error::AppError::BadRequest(
            "code_verifier validation failed".to_string(),
        ));
    }

    let (access_token, id_token, refresh_token) =
        OidcService::issue_authorization_code_tokens(state, &entry, &client).await?;

    Ok(Json(TokenResponse {
        access_token,
        id_token: Some(id_token),
        token_type: "Bearer".to_string(),
        expires_in: state.config.auth.exp_seconds,
        refresh_token: Some(refresh_token),
    }))
}

async fn handle_refresh_token(state: &AppState, req: &TokenRequest) -> Result<Json<TokenResponse>> {
    let rt = req
        .refresh_token
        .as_deref()
        .ok_or(crate::error::AppError::BadRequest(
            "missing refresh_token".to_string(),
        ))?;

    let db = &state.database.connection;

    // Authenticate client
    let client = OidcService::authenticate_client(
        db,
        req.client_id.as_deref(),
        req.client_secret.as_deref(),
    )
    .await?;
    if !OidcService::client_allows_grant_type(&client, "refresh_token") {
        return Err(crate::error::AppError::BadRequest(
            "client not allowed refresh_token grant".to_string(),
        ));
    }

    let rec = crate::entity::oauth_token::Entity::find_by_refresh_token(db, rt)
        .await?
        .ok_or(crate::error::AppError::Unauthorized(
            "invalid refresh_token".to_string(),
        ))?;

    if Utc::now().naive_utc() > rec.refresh_expires_at {
        return Err(crate::error::AppError::Unauthorized(
            "refresh_token expired".to_string(),
        ));
    }
    if rec.client_id != client.client_id {
        return Err(crate::error::AppError::Unauthorized(
            "refresh_token does not belong to this client".to_string(),
        ));
    }

    let (access_token, id_token, new_refresh_token) =
        OidcService::issue_refresh_tokens(state, rt, &rec).await?;

    Ok(Json(TokenResponse {
        access_token,
        id_token: Some(id_token),
        token_type: "Bearer".to_string(),
        expires_in: state.config.auth.exp_seconds,
        refresh_token: Some(new_refresh_token),
    }))
}

async fn handle_client_credentials(
    state: &AppState,
    req: &TokenRequest,
) -> Result<Json<TokenResponse>> {
    let db = &state.database.connection;

    // Authenticate client
    let client = OidcService::authenticate_client(
        db,
        req.client_id.as_deref(),
        req.client_secret.as_deref(),
    )
    .await?;
    if !OidcService::client_allows_grant_type(&client, "client_credentials") {
        return Err(crate::error::AppError::BadRequest(
            "client not allowed client_credentials grant".to_string(),
        ));
    }

    // Validate requested scopes
    if let Some(scope) = &req.scope {
        let req_scopes: Vec<String> = scope
            .split(' ')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !OidcService::client_allows_scopes(&client, &req_scopes) {
            return Err(crate::error::AppError::BadRequest(
                "requested scope not allowed by client".to_string(),
            ));
        }
    }

    let access_token = OidcService::issue_client_credentials_token(state, &client)?;

    Ok(Json(TokenResponse {
        access_token,
        id_token: None,
        token_type: "Bearer".to_string(),
        expires_in: state.config.auth.exp_seconds,
        refresh_token: None,
    }))
}

async fn revoke(
    State(state): State<AppState>,
    Json(req): Json<RevokeRequest>,
) -> Result<http::StatusCode> {
    let db = &state.database.connection;
    OidcService::authenticate_client(db, req.client_id.as_deref(), req.client_secret.as_deref())
        .await?;

    match req.token_type_hint.as_str() {
        "refresh_token" => {
            OidcService::revoke_by_refresh_token(db, &req.token).await?;
        }
        "access_token" => {
            OidcService::revoke_by_access_token(db, &req.token).await?;
        }
        _ => {
            OidcService::revoke_by_refresh_token(db, &req.token).await?;
            OidcService::revoke_by_access_token(db, &req.token).await?;
        }
    }
    Ok(http::StatusCode::OK)
}

async fn introspect(
    State(state): State<AppState>,
    Json(req): Json<IntrospectRequest>,
) -> Result<Json<IntrospectResponse>> {
    let db = &state.database.connection;
    OidcService::authenticate_client(db, req.client_id.as_deref(), req.client_secret.as_deref())
        .await?;

    let inactive = IntrospectResponse {
        active: false,
        scope: None,
        client_id: None,
        username: None,
        token_type: None,
        exp: None,
        sub: None,
    };

    let rec = match OidcService::find_token_record(db, &req.token).await? {
        Some(r) => r,
        None => return Ok(Json(inactive)),
    };

    if rec.revoked {
        return Ok(Json(inactive));
    }

    let now = Utc::now().naive_utc();
    let is_access = rec.access_token == req.token;
    let expired = if is_access {
        now > rec.access_expires_at
    } else {
        now > rec.refresh_expires_at
    };
    if expired {
        return Ok(Json(inactive));
    }

    Ok(Json(IntrospectResponse {
        active: true,
        scope: rec.scope.clone(),
        client_id: Some(rec.client_id.clone()),
        username: None,
        token_type: Some(rec.token_type.clone()),
        exp: Some(if is_access {
            rec.access_expires_at.and_utc().timestamp()
        } else {
            rec.refresh_expires_at.and_utc().timestamp()
        }),
        sub: Some(rec.user_id.to_string()),
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
        preferred_username: Some(user_vo.username),
        name: user_vo.nickname.clone(),
        nickname: user_vo.nickname,
        email: None,
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
    if !OidcService::client_has_redirect(&client, &redirect_uri) {
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
    if !OidcService::client_allows_scopes(&client, &req_scopes) {
        return Err(crate::error::AppError::Unauthorized(
            "requested scope not allowed by client".to_string(),
        ));
    }
    if !OidcService::scopes_exist(db, &req_scopes).await? {
        return Err(crate::error::AppError::Unauthorized(
            "requested scope does not exist".to_string(),
        ));
    }

    // Generate and persist authorization code via service
    let code = OidcService::create_authorization_code(
        db,
        CreateAuthCodeParams {
            user_id: claims.id,
            client_id: client_id.clone(),
            redirect_uri: Some(redirect_uri.clone()),
            code_challenge: q.code_challenge.clone(),
            code_challenge_method: method,
            scope: q.scope.clone(),
            state: q.state.clone(),
            nonce: q.nonce.clone(),
        },
    )
    .await?;

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
