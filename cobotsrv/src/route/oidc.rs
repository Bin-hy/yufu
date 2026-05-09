use axum::extract::State;
use axum::response::Redirect;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::entity::oauth_client;
use crate::result::Result;
use crate::service::oidc::{CreateAuthCodeParams, OidcService, get_issuer_base};
use crate::{AppState, IdType, parse_user_id};
use auth::claims::AccessTokenClaims;
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
    pub code_challenge: Option<String>,
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

// --- Consent flow types ---

#[derive(Serialize)]
pub struct ScopeInfoResp {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct ClientInfoResp {
    pub client_name: String,
    pub client_id: String,
    pub scopes: Vec<ScopeInfoResp>,
    pub already_consented: bool,
}

#[derive(Deserialize)]
pub struct ClientInfoQuery {
    pub client_id: String,
    pub scope: Option<String>,
}

#[derive(Deserialize)]
pub struct ConsentRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub code_challenge: Option<String>,
    #[serde(default = "default_code_challenge_method")]
    pub code_challenge_method: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub approved: bool,
}

#[derive(Serialize)]
pub struct ConsentResponse {
    pub redirect_url: String,
}

#[derive(Serialize)]
pub struct AuthorizedAppVO {
    pub consent_id: i32,
    pub client_id: String,
    pub client_name: String,
    pub scope: String,
    pub authorized_at: String,
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
        .route("/oidc/authorize", get(authorize))
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/userinfo", get(userinfo))
        .route("/oidc/authorize/client-info", get(authorize_client_info))
        .route("/oidc/authorize/consent", post(authorize_consent))
        .route("/oidc/authorized-apps", get(list_authorized_apps))
        .route(
            "/oidc/authorized-apps/{consent_id}",
            delete(revoke_authorized_app),
        )
}

// --- Handlers ---

async fn openid_configuration(State(state): State<AppState>) -> Result<Json<OidcDiscovery>> {
    let base = get_issuer_base(&state);
    let resp = OidcDiscovery {
        issuer: base.clone(),
        authorization_endpoint: format!("{}/oidc/authorize", base),
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
        token_endpoint_auth_methods_supported: vec![
            "client_secret_post".to_string(),
            "client_secret_basic".to_string(),
        ],
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

/// Extract client_id and client_secret from HTTP Basic Auth header.
/// Returns (client_id, client_secret) if present.
fn extract_basic_auth(headers: &http::HeaderMap) -> (Option<String>, Option<String>) {
    let result = (|| {
        let auth = headers.get(http::header::AUTHORIZATION)?;
        let auth_str = auth.to_str().ok()?;
        let encoded = auth_str.strip_prefix("Basic ")?;
        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded.trim())
                .ok()?;
        let credentials = String::from_utf8(decoded).ok()?;
        let (id, secret) = credentials.split_once(':')?;
        Some((id.to_string(), secret.to_string()))
    })();
    match result {
        Some((id, secret)) => (Some(id), Some(secret)),
        None => (None, None),
    }
}

/// Unified token endpoint — dispatches by grant_type
/// Accepts application/x-www-form-urlencoded (OAuth2 standard).
/// Supports client auth via POST body or HTTP Basic Auth.
async fn token_endpoint(
    State(state): State<AppState>,
    headers: http::HeaderMap,
    axum::extract::Form(mut req): axum::extract::Form<TokenRequest>,
) -> Result<Json<TokenResponse>> {
    // If client_id/client_secret not in body, try HTTP Basic Auth header
    if req.client_id.is_none() || req.client_secret.is_none() {
        let (basic_id, basic_secret) = extract_basic_auth(&headers);
        if req.client_id.is_none() {
            req.client_id = basic_id;
        }
        if req.client_secret.is_none() {
            req.client_secret = basic_secret;
        }
    }

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
    let code_verifier = req.code_verifier.as_deref();

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

    // PKCE (code_challenge present) provides proof-of-possession — public clients
    // (SPA) don't need client_secret. Non-PKCE flows still require secret auth.
    let pkce_used = !entry.code_challenge.is_empty();
    let client = if pkce_used {
        let cid = req
            .client_id
            .as_deref()
            .ok_or(crate::error::AppError::BadRequest(
                "missing client_id".to_string(),
            ))?;
        oauth_client::Entity::get_by_client_id(db, cid)
            .await?
            .ok_or(crate::error::AppError::Unauthorized(
                "invalid client_id".to_string(),
            ))?
    } else {
        OidcService::authenticate_client(db, req.client_id.as_deref(), req.client_secret.as_deref())
            .await?
    };

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
    if pkce_used {
        let verifier = code_verifier.ok_or(crate::error::AppError::BadRequest(
            "missing code_verifier (PKCE was used during authorization)".to_string(),
        ))?;
        if !OidcService::verify_pkce(
            &entry.code_challenge,
            &entry.code_challenge_method,
            verifier,
        ) {
            return Err(crate::error::AppError::BadRequest(
                "code_verifier validation failed".to_string(),
            ));
        }
    }

    let (access_token, id_token, refresh_token) =
        OidcService::issue_authorization_code_tokens(state, &entry, &client).await?;

    Ok(Json(TokenResponse {
        access_token,
        id_token: Some(id_token),
        token_type: "Bearer".to_string(),
        expires_in: state.oidc_access_token_exp(),
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
        expires_in: state.oidc_access_token_exp(),
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
        expires_in: state.oidc_access_token_exp(),
        refresh_token: None,
    }))
}

async fn revoke(
    State(state): State<AppState>,
    axum::extract::Form(req): axum::extract::Form<RevokeRequest>,
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
    axum::extract::Form(req): axum::extract::Form<IntrospectRequest>,
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
    claims: AccessTokenClaims,
    State(state): State<AppState>,
) -> Result<Json<UserInfoResp>> {
    let db = &state.database.connection;
    let user_id = parse_user_id(&claims.sub)?;
    let user_vo = crate::service::user::UserService::get_user_by_id(db, user_id).await?;

    let email_domain = &state.config.oidc.email_domain;
    Ok(Json(UserInfoResp {
        sub: user_vo.id,
        preferred_username: Some(user_vo.username.clone()),
        name: user_vo.nickname.clone(),
        nickname: user_vo.nickname,
        email: Some(format!("{}@{}", user_vo.username, email_domain)),
    }))
}

/// Build a query string from authorize params (for redirect URLs).
fn build_authorize_query_string(q: &AuthorizeQuery) -> String {
    let mut parts = vec![
        format!("client_id={}", urlencoding::encode(&q.client_id)),
        format!("redirect_uri={}", urlencoding::encode(&q.redirect_uri)),
        format!("response_type={}", urlencoding::encode(&q.response_type)),
    ];
    if let Some(ref cc) = q.code_challenge {
        parts.push(format!("code_challenge={}", urlencoding::encode(cc)));
        parts.push(format!(
            "code_challenge_method={}",
            urlencoding::encode(&q.code_challenge_method)
        ));
    }
    if let Some(ref scope) = q.scope {
        parts.push(format!("scope={}", urlencoding::encode(scope)));
    }
    if let Some(ref state) = q.state {
        parts.push(format!("state={}", urlencoding::encode(state)));
    }
    if let Some(ref nonce) = q.nonce {
        parts.push(format!("nonce={}", urlencoding::encode(nonce)));
    }
    parts.join("&")
}

async fn authorize(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<AuthorizeQuery>,
) -> Result<Redirect> {
    if q.response_type != "code" {
        return Err(crate::error::AppError::BadRequest(
            "unsupported response_type, expected code".to_string(),
        ));
    }

    if q.code_challenge.is_some() {
        let method = q.code_challenge_method.clone();
        if method != "S256" && method != "plain" {
            return Err(crate::error::AppError::BadRequest(
                "unsupported code_challenge_method".to_string(),
            ));
        }
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

    // Try to extract JWT from Authorization header (manually, since we're not behind auth middleware)
    let qs = build_authorize_query_string(&q);
    let user_id: Option<IdType> = {
        // Try to read token from Authorization header via request extensions
        // Since we're in a no-prefix route, we manually verify the token
        // The token should be passed as a cookie by the SPA
        None // Will be set below if we can extract from headers
    };

    // For this no-auth route, we can't directly access request headers from the handler signature
    // with axum extractors in a clean way without changing the signature.
    // Instead, we use an alternative approach: the frontend SPA handles the flow.
    //
    // Flow:
    // 1. Third-party app redirects user to /oidc/authorize?...
    // 2. This handler ALWAYS redirects to the frontend at /oidc/consent?...
    //    (the frontend will check if user is logged in via its router guard)
    // 3. The frontend consent page calls /api/v1/oidc/authorize/client-info to get app info
    // 4. The frontend consent page calls POST /api/v1/oidc/authorize/consent to approve/deny
    //
    // This is simpler and more aligned with the SPA architecture.

    let _ = user_id; // suppress unused warning
    let frontend_url = format!("/oidc/consent?{}", qs);
    Ok(Redirect::to(&frontend_url))
}

/// GET /api/v1/oidc/authorize/client-info — returns client info for the consent page
async fn authorize_client_info(
    claims: AccessTokenClaims,
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<ClientInfoQuery>,
) -> Result<Json<ClientInfoResp>> {
    let db = &state.database.connection;

    let client = crate::entity::oauth_client::Entity::get_by_client_id(db, &q.client_id)
        .await?
        .ok_or(crate::error::AppError::NotFound(
            "client not found".to_string(),
        ))?;

    let scope_str = q.scope.unwrap_or_else(|| "openid".to_string());
    let scope_names: Vec<String> = scope_str
        .split(' ')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let scopes_with_desc = OidcService::get_scopes_with_descriptions(db, &scope_names).await?;
    let scopes: Vec<ScopeInfoResp> = scopes_with_desc
        .into_iter()
        .map(|(name, desc)| ScopeInfoResp {
            name,
            description: desc,
        })
        .collect();

    let user_id = parse_user_id(&claims.sub)?;
    let already_consented =
        if let Some(consent) = OidcService::find_consent(db, user_id, &q.client_id).await? {
            OidcService::consent_covers_scopes(&consent, &scope_names)
        } else {
            false
        };

    Ok(Json(ClientInfoResp {
        client_name: client.name,
        client_id: client.client_id,
        scopes,
        already_consented,
    }))
}

/// POST /api/v1/oidc/authorize/consent — approve or deny authorization
async fn authorize_consent(
    claims: AccessTokenClaims,
    State(state): State<AppState>,
    Json(req): Json<ConsentRequest>,
) -> Result<Json<ConsentResponse>> {
    let db = &state.database.connection;

    if !req.approved {
        // User denied — redirect with error
        let mut url = req.redirect_uri;
        let sep = if url.contains('?') { '&' } else { '?' };
        url.push(sep);
        url.push_str("error=access_denied");
        if let Some(st) = req.state {
            url.push_str(&format!("&state={}", st));
        }
        return Ok(Json(ConsentResponse { redirect_url: url }));
    }

    // Validate request
    if req.response_type != "code" {
        return Err(crate::error::AppError::BadRequest(
            "unsupported response_type".to_string(),
        ));
    }

    let client = crate::entity::oauth_client::Entity::get_by_client_id(db, &req.client_id)
        .await?
        .ok_or(crate::error::AppError::Unauthorized(
            "invalid client_id".to_string(),
        ))?;
    if !OidcService::client_has_redirect(&client, &req.redirect_uri) {
        return Err(crate::error::AppError::Unauthorized(
            "invalid redirect_uri".to_string(),
        ));
    }

    let scope_str = req.scope.clone().unwrap_or_else(|| "openid".to_string());
    let req_scopes: Vec<String> = scope_str
        .split(' ')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !OidcService::client_allows_scopes(&client, &req_scopes) {
        return Err(crate::error::AppError::Unauthorized(
            "requested scope not allowed".to_string(),
        ));
    }

    let user_id = parse_user_id(&claims.sub)?;

    // Save consent
    OidcService::save_consent(db, user_id, &req.client_id, &scope_str).await?;

    // Generate authorization code
    let code = OidcService::create_authorization_code(
        db,
        CreateAuthCodeParams {
            user_id,
            client_id: req.client_id,
            redirect_uri: Some(req.redirect_uri.clone()),
            code_challenge: req.code_challenge,
            code_challenge_method: req.code_challenge_method,
            scope: req.scope,
            state: req.state.clone(),
            nonce: req.nonce,
        },
    )
    .await?;

    // Build redirect URL
    let mut url = req.redirect_uri;
    let sep = if url.contains('?') { '&' } else { '?' };
    url.push(sep);
    url.push_str(&format!("code={}", code));
    if let Some(st) = req.state {
        url.push_str(&format!("&state={}", st));
    }

    Ok(Json(ConsentResponse { redirect_url: url }))
}

/// GET /api/v1/oidc/authorized-apps — list user's authorized apps
async fn list_authorized_apps(
    claims: AccessTokenClaims,
    State(state): State<AppState>,
) -> Result<Json<Vec<AuthorizedAppVO>>> {
    let db = &state.database.connection;
    let user_id = parse_user_id(&claims.sub)?;
    let consents = OidcService::list_user_consents(db, user_id).await?;

    let apps: Vec<AuthorizedAppVO> = consents
        .into_iter()
        .filter_map(|(consent, client_opt)| {
            let client = client_opt?;
            Some(AuthorizedAppVO {
                consent_id: consent.id,
                client_id: client.client_id,
                client_name: client.name,
                scope: consent.scope,
                authorized_at: consent.create_time.to_string(),
            })
        })
        .collect();

    Ok(Json(apps))
}

/// DELETE /api/v1/oidc/authorized-apps/:consent_id — revoke user's authorization
async fn revoke_authorized_app(
    claims: AccessTokenClaims,
    State(state): State<AppState>,
    axum::extract::Path(consent_id): axum::extract::Path<i32>,
) -> Result<http::StatusCode> {
    let user_id = parse_user_id(&claims.sub)?;
    let db = &state.database.connection;
    OidcService::revoke_consent(db, consent_id, user_id).await?;
    Ok(http::StatusCode::OK)
}
