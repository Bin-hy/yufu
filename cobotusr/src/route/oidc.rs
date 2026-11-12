use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::entity::oauth_client;
use crate::result::Result;
use crate::service::oidc::{CreateAuthCodeParams, OidcService, get_issuer_base};
use crate::service::security::{browser, revocation::RevocationReceipt};
use crate::{AppState, parse_user_id};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_session_endpoint: Option<String>,
    pub response_types_supported: Vec<String>,
    pub response_modes_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub introspection_endpoint_auth_methods_supported: Vec<String>,
    pub revocation_endpoint_auth_methods_supported: Vec<String>,
    pub scopes_supported: Vec<String>,
    pub claims_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backchannel_logout_supported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backchannel_logout_session_supported: Option<bool>,
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

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
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
        .route(
            "/oidc/token",
            post(super::identity_token::token).layer(axum::extract::DefaultBodyLimit::max(16_384)),
        )
        .route(
            "/oidc/revoke",
            post(super::identity_status::revoke)
                .layer(axum::extract::DefaultBodyLimit::max(16_384)),
        )
        .route(
            "/oidc/introspect",
            post(super::identity_status::introspect)
                .layer(axum::extract::DefaultBodyLimit::max(16_384)),
        )
        .route(
            "/oidc/userinfo",
            get(super::identity_userinfo::userinfo).post(super::identity_userinfo::userinfo),
        )
        .route("/oidc/logout", get(end_session).post(confirm_end_session))
        .route(
            "/oidc/end-session",
            get(super::identity_logout::get)
                .post(super::identity_logout::post)
                .layer(axum::extract::DefaultBodyLimit::max(16_384)),
        )
        .route(
            "/oidc/authorize",
            get(super::identity_authorization::authorize),
        )
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
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
    let supports_backchannel_logout = state.id_token_signer.algorithm() == "RS256";
    let resp = OidcDiscovery {
        issuer: base.clone(),
        authorization_endpoint: format!("{}/oidc/authorize", base),
        token_endpoint: format!("{}/oidc/token", base),
        userinfo_endpoint: format!("{}/oidc/userinfo", base),
        jwks_uri: Some(format!("{}/.well-known/jwks.json", base)),
        revocation_endpoint: format!("{}/oidc/revoke", base),
        introspection_endpoint: format!("{}/oidc/introspect", base),
        end_session_endpoint: supports_backchannel_logout
            .then(|| format!("{}/oidc/end-session", base)),
        response_types_supported: vec!["code".to_string()],
        response_modes_supported: vec!["query".into()],
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: vec![state.id_token_signer.algorithm().to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        token_endpoint_auth_methods_supported: vec![
            "client_secret_post".to_string(),
            "client_secret_basic".to_string(),
            "none".to_string(),
        ],
        introspection_endpoint_auth_methods_supported: vec![
            "client_secret_basic".into(),
            "client_secret_post".into(),
        ],
        revocation_endpoint_auth_methods_supported: vec![
            "client_secret_basic".into(),
            "client_secret_post".into(),
            "none".into(),
        ],
        scopes_supported: vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
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
            "email".to_string(),
            "email_verified".into(),
            "auth_time".into(),
            "sid".into(),
        ],
        code_challenge_methods_supported: vec!["S256".to_string()],
        backchannel_logout_supported: Some(supports_backchannel_logout),
        backchannel_logout_session_supported: Some(supports_backchannel_logout),
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
/// Accepts application/x-www-form-urlencoded (OAuth2 standard).
pub(crate) async fn handle_authorization_code(
    state: &AppState,
    req: &TokenRequest,
    budget: &SecurityBudget,
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
    let set = OidcService::exchange_authorization_code_with_budget(
        state,
        CodeExchangeRequest {
            code: &code,
            client_id,
            client_secret: secret.as_ref(),
            redirect_uri: req.redirect_uri.as_deref(),
            code_verifier: verifier.as_ref(),
        },
        budget,
    )
    .await?;
    Ok(Json(session_token_response(state, set)))
}

pub(crate) async fn handle_refresh_token(
    state: &AppState,
    req: &TokenRequest,
    budget: &SecurityBudget,
) -> Result<Json<TokenResponse>> {
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
    let set = OidcService::rotate_session_refresh_with_budget(
        state,
        RefreshRequest {
            refresh_token: &refresh_token,
            client_id,
            client_secret: secret.as_ref(),
        },
        budget,
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

pub(crate) async fn handle_client_credentials(
    state: &AppState,
    req: &TokenRequest,
    budget: &SecurityBudget,
    request_id: uuid::Uuid,
) -> Result<Json<TokenResponse>> {
    let client_id = req
        .client_id
        .as_deref()
        .ok_or(SecurityError::InvalidCredential)?;
    let secret = SecretToken::new(
        req.client_secret
            .clone()
            .ok_or(SecurityError::InvalidCredential)?,
    )?;
    let signing_state = state.clone();
    let access_token = crate::service::identity::legacy_client::execute(
        state,
        client_id,
        &secret,
        None,
        "client_credentials",
        request_id,
        budget,
        |_, client, _| {
            let client = client.clone();
            let scopes = req.scope.clone();
            let state = signing_state.clone();
            Box::pin(async move {
                if let Some(scope) = scopes {
                    let scopes = scope
                        .split_whitespace()
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    if !OidcService::client_allows_scopes(&client, &scopes) {
                        return Err(crate::error::AppError::BadRequest(
                            "requested scope not allowed by client".into(),
                        ));
                    }
                }
                OidcService::issue_client_credentials_token(&state, &client)
            })
        },
    )
    .await?;
    Ok(Json(TokenResponse {
        access_token,
        id_token: None,
        token_type: "Bearer".to_string(),
        expires_in: state.oidc_access_token_exp(),
        refresh_token: None,
    }))
}

pub(crate) async fn revoke_legacy(
    State(state): State<AppState>,
    axum::extract::Form(req): axum::extract::Form<RevokeRequest>,
    budget: &SecurityBudget,
) -> Result<Json<RevocationReceipt>> {
    // A hint is advisory; registered refresh history and signed access evidence
    // are both tried regardless of its value.
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
                budget,
            )
            .await?,
    ))
}

pub(crate) async fn introspect_legacy(
    State(state): State<AppState>,
    axum::extract::Form(req): axum::extract::Form<IntrospectRequest>,
) -> Result<Json<IntrospectResponse>> {
    let db = &state.database.connection;
    let client = OidcService::authenticate_client(
        db,
        req.client_id.as_deref(),
        req.client_secret.as_deref(),
    )
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

    if rec.revoked || rec.client_id != client.client_id {
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

pub(super) fn html_attribute(value: &str) -> String {
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    State(state): State<AppState>,
    Json(req): Json<ConsentRequest>,
) -> Result<Json<ConsentResponse>> {
    use crate::service::identity::legacy_governance::{MutationScope, mutate};
    let target = oauth_client::Entity::get_by_client_id(&state.database.connection, &req.client_id)
        .await?
        .ok_or_else(|| crate::error::AppError::Unauthorized("invalid client_id".into()))?;
    let user_id = parse_user_id(&claims.sub)?;
    mutate(
        &state,
        &context,
        "POST",
        "/oidc/authorize/consent",
        MutationScope::ClientConsent(target.id),
        |db| {
            let req = req.clone();
            Box::pin(async move {
                let client = oauth_client::Entity::get_by_client_id(db, &req.client_id)
                    .await?
                    .ok_or_else(|| {
                        crate::error::AppError::Unauthorized("invalid client_id".into())
                    })?;
                if client.management_state != "legacy_unmanaged" {
                    return Err(crate::error::AppError::BadRequest(
                        "请通过标准授权入口建立并完成授权流程".into(),
                    ));
                }
                if !OidcService::client_has_redirect(&client, &req.redirect_uri) {
                    return Err(crate::error::AppError::Unauthorized(
                        "invalid redirect_uri".into(),
                    ));
                }
                let mut url = url::Url::parse(&req.redirect_uri).map_err(|_| {
                    crate::error::AppError::BadRequest("invalid redirect_uri".into())
                })?;
                if !req.approved {
                    url.query_pairs_mut().append_pair("error", "access_denied");
                } else {
                    if req.response_type != "code" {
                        return Err(crate::error::AppError::BadRequest(
                            "unsupported response_type".into(),
                        ));
                    }
                    if !OidcService::public_client_requires_s256_pkce(
                        &client,
                        req.code_challenge.as_deref(),
                        &req.code_challenge_method,
                    ) {
                        return Err(crate::error::AppError::BadRequest(
                            "public clients require a non-empty S256 code_challenge".into(),
                        ));
                    }
                    let scope = req.scope.as_deref().unwrap_or("openid");
                    let scopes = scope
                        .split_whitespace()
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    if !OidcService::client_allows_scopes(&client, &scopes) {
                        return Err(crate::error::AppError::Unauthorized(
                            "requested scope not allowed".into(),
                        ));
                    }
                    OidcService::save_consent(db, user_id, &req.client_id, scope).await?;
                    let code = OidcService::create_authorization_code(
                        db,
                        CreateAuthCodeParams {
                            user_id,
                            client_id: req.client_id,
                            redirect_uri: Some(req.redirect_uri),
                            code_challenge: req.code_challenge,
                            code_challenge_method: req.code_challenge_method,
                            scope: req.scope,
                            state: req.state.clone(),
                            nonce: req.nonce,
                        },
                    )
                    .await?;
                    url.query_pairs_mut().append_pair("code", &code);
                }
                if let Some(state) = req.state {
                    url.query_pairs_mut().append_pair("state", &state);
                }
                Ok(Json(ConsentResponse {
                    redirect_url: url.into(),
                }))
            })
        },
    )
    .await
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
