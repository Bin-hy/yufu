use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, Redirect};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::entity::oauth_client;
use crate::result::Result;
use crate::service::oidc::{CreateAuthCodeParams, OidcService, get_issuer_base};
use crate::service::security::{browser, revocation::RevocationReceipt};
use crate::{AppState, IdType, parse_user_id};
use auth::claims::AccessTokenClaims;
use auth::security::{budget::SecurityBudget, error::SecurityError, identity::SecretToken};
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
    pub end_session_endpoint: String,
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
    pub sub: String,
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

#[derive(Deserialize)]
pub struct EndSessionQuery {
    pub client_id: String,
    pub post_logout_redirect_uri: String,
}

#[derive(Deserialize)]
struct EndSessionForm {
    client_id: String,
    post_logout_redirect_uri: String,
    csrf_token: Option<String>,
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
        .route("/oidc/logout", get(end_session).post(confirm_end_session))
        .route("/oidc/authorize", get(authorize))
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/userinfo", get(userinfo))
        .route("/oidc/authorize/client-info", get(authorize_client_info))
        .route("/oidc/authorize/consent", post(authorize_consent))
        .route("/oidc/authorized-apps", get(list_authorized_apps))
}

pub fn get_revocation_routes() -> Router<AppState> {
    Router::new().route(
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
        end_session_endpoint: format!("{}/oidc/logout", base),
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
            "none".to_string(),
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
    use crate::service::security::lifecycle::CodeExchangeRequest;
    use auth::security::identity::SecretToken;
    let code = SecretToken::new(
        req.code
            .clone()
            .ok_or(crate::error::AppError::BadRequest("missing code".into()))?,
    )?;
    let client_id = req
        .client_id
        .as_deref()
        .ok_or(crate::error::AppError::BadRequest(
            "missing client_id".into(),
        ))?;
    let secret = req
        .client_secret
        .clone()
        .map(SecretToken::new)
        .transpose()?;
    let verifier = req
        .code_verifier
        .clone()
        .map(SecretToken::new)
        .transpose()?;
    let set = OidcService::exchange_authorization_code(
        state,
        CodeExchangeRequest {
            code: &code,
            client_id,
            client_secret: secret.as_ref(),
            redirect_uri: req.redirect_uri.as_deref(),
            code_verifier: verifier.as_ref(),
        },
    )
    .await?;
    Ok(Json(session_token_response(state, set)))
}

async fn handle_refresh_token(state: &AppState, req: &TokenRequest) -> Result<Json<TokenResponse>> {
    use crate::service::security::lifecycle::RefreshRequest;
    use auth::security::identity::SecretToken;
    let refresh_token = SecretToken::new(req.refresh_token.clone().ok_or(
        crate::error::AppError::BadRequest("missing refresh_token".into()),
    )?)?;
    let client_id = req
        .client_id
        .as_deref()
        .ok_or(crate::error::AppError::BadRequest(
            "missing client_id".into(),
        ))?;
    let secret = req
        .client_secret
        .clone()
        .map(SecretToken::new)
        .transpose()?;
    let set = OidcService::rotate_session_refresh(
        state,
        RefreshRequest {
            refresh_token: &refresh_token,
            client_id,
            client_secret: secret.as_ref(),
        },
    )
    .await?;
    Ok(Json(session_token_response(state, set)))
}

fn session_token_response(
    state: &AppState,
    set: crate::service::security::lifecycle::SessionTokenSet,
) -> TokenResponse {
    TokenResponse {
        access_token: set.access_token.expose_secret().into(),
        id_token: set.id_token.map(|token| token.expose_secret().into()),
        refresh_token: Some(set.refresh_token.expose_secret().into()),
        token_type: "Bearer".into(),
        expires_in: state.oidc_access_token_exp(),
    }
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
    headers: HeaderMap,
    axum::extract::Form(mut req): axum::extract::Form<RevokeRequest>,
) -> Result<Json<RevocationReceipt>> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    // A hint is advisory; registered refresh history and signed access evidence
    // are both tried regardless of its value.
    normalize_revocation_client_auth(&headers, &mut req)?;
    let client_id = req
        .client_id
        .as_deref()
        .ok_or(crate::error::AppError::BadRequest(
            "missing client_id".into(),
        ))?;
    let token = SecretToken::new(req.token)?;
    let secret = req.client_secret.map(SecretToken::new).transpose()?;
    let reference = if token.expose_secret().split('.').count() == 3 {
        let (signer, issuer, evidence) = (
            state.id_token_signer().clone(),
            state.issuer(),
            SecretToken::new(token.expose_secret().to_owned())?,
        );
        match budget
            .run(tokio::task::spawn_blocking(move || {
                signer.revocation_reference(&evidence, &issuer)
            }))
            .await?
            .map_err(|_| SecurityError::DependencyUnavailable)?
        {
            Ok(reference) => Some(reference),
            Err(SecurityError::InvalidCredential) => None,
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };
    Ok(Json(
        browser::lifecycle(&state)?
            .revoke_oidc(
                crate::service::security::revocation::OidcRevocationRequest {
                    token: &token,
                    client_id,
                    client_secret: secret.as_ref(),
                    access_reference: reference.as_ref(),
                },
                &state.issuer(),
                &budget,
            )
            .await?,
    ))
}

fn normalize_revocation_client_auth(headers: &HeaderMap, req: &mut RevokeRequest) -> Result<()> {
    if let Some(value) = browser::single_header(headers, "authorization")? {
        if !value.starts_with("Basic ") {
            return Err(SecurityError::InvalidCredential.into());
        }
        let (id, secret) = extract_basic_auth(headers);
        let id = id.ok_or(SecurityError::InvalidCredential)?;
        let secret = secret.ok_or(SecurityError::InvalidCredential)?;
        if req.client_secret.is_some() || req.client_id.as_ref().is_some_and(|body| body != &id) {
            return Err(crate::error::AppError::BadRequest(
                "conflicting client authentication".into(),
            ));
        }
        req.client_id = Some(id);
        req.client_secret = Some(secret);
    }
    Ok(())
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
        sub: user_vo.id.to_string(),
        preferred_username: Some(user_vo.username.clone()),
        name: user_vo.nickname.clone(),
        nickname: user_vo.nickname,
        email: Some(format!("{}@{}", user_vo.username, email_domain)),
    }))
}

fn html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// GET only prepares a confirmation; it never revokes or clears a session.
async fn end_session(
    State(state): State<AppState>,
    Query(query): Query<EndSessionQuery>,
    request_headers: HeaderMap,
) -> Result<(HeaderMap, Html<String>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let client = budget
        .run(oauth_client::Entity::get_by_client_id(
            &state.database.connection,
            &query.client_id,
        ))
        .await?
        .map_err(|_| SecurityError::DependencyUnavailable)?
        .ok_or(SecurityError::InvalidCredential)?;
    if !OidcService::client_has_redirect(&client, &query.post_logout_redirect_uri) {
        return Err(SecurityError::InvalidResource.into());
    }
    let cookies = browser::cookies(&request_headers)?;
    let mut headers = HeaderMap::new();
    let proof = browser::issue_csrf(
        &state,
        cookies.get("refresh_token").map(String::as_str),
        &mut headers,
    )?;
    headers.insert("x-frame-options", http::HeaderValue::from_static("DENY"));
    let body = format!(
        "<!doctype html><html><body><form method=\"post\" action=\"/oidc/logout\"><input type=\"hidden\" name=\"client_id\" value=\"{}\"><input type=\"hidden\" name=\"post_logout_redirect_uri\" value=\"{}\"><input type=\"hidden\" name=\"csrf_token\" value=\"{}\"><button type=\"submit\">确认退出当前会话</button></form></body></html>",
        html_attribute(&query.client_id),
        html_attribute(&query.post_logout_redirect_uri),
        proof.expose_secret()
    );
    budget.remaining()?;
    Ok((headers, Html(body)))
}

async fn confirm_end_session(
    State(state): State<AppState>,
    request_headers: HeaderMap,
    axum::extract::Form(form): axum::extract::Form<EndSessionForm>,
) -> Result<(HeaderMap, Html<String>)> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let refresh = browser::validate_csrf(
        &state,
        &request_headers,
        form.csrf_token.as_deref(),
        &budget,
    )?
    .ok_or(crate::error::AppError::BadRequest(
        "missing refresh token".into(),
    ))?;
    let receipt = crate::service::user::UserService::logout_by_refresh(
        &state,
        &refresh,
        Some((&form.client_id, &form.post_logout_redirect_uri)),
        &budget,
    )
    .await?;
    let redirect = serde_json::to_string(&form.post_logout_redirect_uri)?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    let body = format!(
        "<!doctype html><html><body data-committed-at=\"{}\" data-effective-by=\"{}\"><script>localStorage.removeItem('userInfo');for(const part of document.cookie.split(';')){{const name=part.trim().split('=',1)[0];if(name==='access_token'||name.endsWith('-token-key'))document.cookie=name+'=; Path=/; Max-Age=0; SameSite=Lax';}}window.location.replace({redirect});</script></body></html>",
        receipt.committed_at, receipt.effective_by
    );
    let mut headers = browser::clear_session_cookies(&state)?;
    headers.insert("x-frame-options", http::HeaderValue::from_static("DENY"));
    Ok((headers, Html(body)))
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
    if !OidcService::public_client_requires_s256_pkce(
        &client,
        q.code_challenge.as_deref(),
        &q.code_challenge_method,
    ) {
        return Err(crate::error::AppError::BadRequest(
            "public clients require a non-empty S256 code_challenge".to_string(),
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
    if !OidcService::public_client_requires_s256_pkce(
        &client,
        req.code_challenge.as_deref(),
        &req.code_challenge_method,
    ) {
        return Err(crate::error::AppError::BadRequest(
            "public clients require a non-empty S256 code_challenge".to_string(),
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
    State(state): State<AppState>,
    request_headers: HeaderMap,
    axum::extract::Path(consent_id): axum::extract::Path<i32>,
) -> Result<Json<RevocationReceipt>> {
    let budget = SecurityBudget::new(state.security_clock()?)?;
    let manager = browser::authenticate_manager(&state, &request_headers, &budget).await?;
    Ok(Json(
        browser::lifecycle(&state)?
            .revoke_consent(&manager, consent_id, &budget)
            .await?,
    ))
}
