use super::identity_browser::IdentityRequest;
use crate::{
    AppState,
    domain::identity::{authorization::strict_form, error::IdentityError},
    service::{
        identity::{
            protocol::ProtocolService,
            repository::{IdentityRepository, PgIdentityRepository},
        },
        security::{
            lifecycle::{CodeExchangeRequest, CredentialLifecycle, RefreshRequest},
            repository::PgSecurityRepository,
        },
    },
};
use auth::security::identity::SecretToken;
use axum::{
    Extension, Json,
    body::Bytes,
    extract::State,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use http::{
    HeaderMap, StatusCode,
    header::{AUTHORIZATION, WWW_AUTHENTICATE},
};
use std::sync::Arc;

pub(crate) fn service(state: &AppState) -> Result<ProtocolService, IdentityError> {
    let clock = state.security_clock()?;
    ProtocolService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        CredentialLifecycle::new(
            Arc::new(PgSecurityRepository::new(
                state.database.connection.clone(),
                clock.clone(),
            )),
            clock.clone(),
        ),
        clock,
        state.id_token_signer().clone(),
        state.config.clone(),
    )
}
#[derive(Debug)]
pub(crate) enum TokenError {
    InvalidRequest,
    InvalidClient,
    InvalidGrant,
    InvalidScope,
    UnsupportedGrant,
    Backend(IdentityError),
}
impl From<IdentityError> for TokenError {
    fn from(error: IdentityError) -> Self {
        match error {
            IdentityError::Unauthenticated => Self::InvalidClient,
            IdentityError::AccessDenied
            | IdentityError::Expired
            | IdentityError::AlreadyConsumed => Self::InvalidGrant,
            IdentityError::InvalidInput => Self::InvalidRequest,
            IdentityError::InvalidScope => Self::InvalidScope,
            error => Self::Backend(error),
        }
    }
}
pub(crate) fn response_error(error: TokenError, id: uuid::Uuid) -> Response {
    let (status, code) = match error {
        TokenError::InvalidRequest => (StatusCode::BAD_REQUEST, "invalid_request"),
        TokenError::InvalidClient => (StatusCode::UNAUTHORIZED, "invalid_client"),
        TokenError::InvalidGrant => (StatusCode::BAD_REQUEST, "invalid_grant"),
        TokenError::InvalidScope => (StatusCode::BAD_REQUEST, "invalid_scope"),
        TokenError::UnsupportedGrant => (StatusCode::BAD_REQUEST, "unsupported_grant_type"),
        TokenError::Backend(reason) => {
            return crate::error::AppError::identity(reason, id).into_response();
        }
    };
    let mut response = (
        status,
        Json(serde_json::json!({"error":code,"request_id":id})),
    )
        .into_response();
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            http::HeaderValue::from_static("Basic realm=\"identity\""),
        );
    }
    response
}
fn decode_basic_component(value: &str) -> Result<String, TokenError> {
    strict_form(&format!("component={value}"), 8192)
        .map_err(|_| TokenError::InvalidClient)?
        .remove("component")
        .ok_or(TokenError::InvalidClient)
}
pub(crate) fn authenticated_form(
    headers: &HeaderMap,
    body: &[u8],
    allowed: &[&str],
) -> Result<std::collections::BTreeMap<String, String>, TokenError> {
    let content_type = crate::service::security::browser::single_header(headers, "content-type")
        .map_err(|_| TokenError::InvalidRequest)?
        .ok_or(TokenError::InvalidRequest)?;
    if !content_type.split(';').next().is_some_and(|kind| {
        kind.trim()
            .eq_ignore_ascii_case("application/x-www-form-urlencoded")
    }) {
        return Err(TokenError::InvalidRequest);
    }
    let body = std::str::from_utf8(body).map_err(|_| TokenError::InvalidRequest)?;
    let mut fields = strict_form(body, 16_384).map_err(|_| TokenError::InvalidRequest)?;
    if fields.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(TokenError::InvalidRequest);
    }
    let basic = crate::service::security::browser::single_header(headers, AUTHORIZATION.as_str())
        .map_err(|_| TokenError::InvalidClient)?;
    if let Some(basic) = basic {
        let (scheme, encoded) = basic.split_once(' ').ok_or(TokenError::InvalidClient)?;
        if !scheme.eq_ignore_ascii_case("Basic") {
            return Err(TokenError::InvalidClient);
        }
        let raw = STANDARD
            .decode(encoded)
            .map_err(|_| TokenError::InvalidClient)?;
        let raw = std::str::from_utf8(&raw).map_err(|_| TokenError::InvalidClient)?;
        let (id, secret) = raw.split_once(':').ok_or(TokenError::InvalidClient)?;
        let id = decode_basic_component(id)?;
        let secret = decode_basic_component(secret)?;
        if fields.contains_key("client_secret")
            || fields.get("client_id").is_some_and(|value| value != &id)
        {
            return Err(TokenError::InvalidRequest);
        }
        fields.insert("client_id".into(), id);
        fields.insert("client_secret".into(), secret);
    }
    if fields.get("client_id").is_none_or(String::is_empty)
        || fields.get("client_secret").is_some_and(String::is_empty)
    {
        return Err(TokenError::InvalidClient);
    }
    Ok(fields)
}
fn parse(headers: &HeaderMap, body: &[u8]) -> Result<super::oidc::TokenRequest, TokenError> {
    let fields = authenticated_form(
        headers,
        body,
        &[
            "grant_type",
            "code",
            "code_verifier",
            "redirect_uri",
            "refresh_token",
            "client_id",
            "client_secret",
            "scope",
        ],
    )?;
    serde_json::from_value(serde_json::json!(fields)).map_err(|_| TokenError::InvalidRequest)
}
pub(crate) async fn token(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let result = process_token(&state, &context, &headers, &body).await;
    let mut response = result.unwrap_or_else(|reason| response_error(reason, context.id));
    for (name, value) in crate::service::identity::browser::protected_headers() {
        if let Some(name) = name {
            response.headers_mut().insert(name, value);
        }
    }
    response
}

async fn process_token(
    state: &AppState,
    context: &IdentityRequest,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Response, TokenError> {
    let request = parse(headers, body)?;
    let origin = crate::service::security::browser::single_header(headers, "origin")
        .map_err(|_| TokenError::InvalidRequest)?;
    if !matches!(
        request.grant_type.as_str(),
        "authorization_code" | "refresh_token" | "client_credentials"
    ) {
        return Err(TokenError::UnsupportedGrant);
    }
    let client_id = request
        .client_id
        .as_deref()
        .ok_or(TokenError::InvalidClient)?;
    let repository = PgIdentityRepository::new(
        state.database.connection.clone(),
        state
            .security_clock()
            .map_err(|error| TokenError::Backend(error.into()))?,
    );
    let mut tx = repository.begin(&context.budget).await?;
    let client = tx
        .oidc_client(client_id)
        .await
        .map_err(|error| TokenError::Backend(error.into()));
    tx.rollback()
        .await
        .map_err(|error| TokenError::Backend(error.into()))?;
    let client = client?.ok_or(TokenError::InvalidClient)?;
    if client.management_state == "legacy_unmanaged" {
        if let Some(origin) = &origin {
            let issuer = url::Url::parse(&state.issuer())
                .map_err(|_| TokenError::Backend(IdentityError::DependencyUnavailable))?;
            if origin != &issuer.origin().ascii_serialization() {
                return Err(TokenError::InvalidRequest);
            }
        }
        let response = match request.grant_type.as_str() {
            "authorization_code" => {
                super::oidc::handle_authorization_code(state, &request, &context.budget).await
            }
            "refresh_token" => {
                super::oidc::handle_refresh_token(state, &request, &context.budget).await
            }
            "client_credentials" => {
                super::oidc::handle_client_credentials(state, &request, &context.budget, context.id)
                    .await
            }
            _ => return Err(TokenError::UnsupportedGrant),
        };
        return Ok(response.into_response());
    }
    if client.management_state != "managed" {
        return Err(TokenError::InvalidClient);
    }
    let secret = request
        .client_secret
        .clone()
        .map(SecretToken::new)
        .transpose()
        .map_err(|_| TokenError::InvalidClient)?;
    let service = service(state)?;
    let issued = match request.grant_type.as_str() {
        "authorization_code" => {
            if request.refresh_token.is_some() || request.scope.is_some() {
                return Err(TokenError::InvalidRequest);
            }
            let code = SecretToken::new(request.code.clone().ok_or(TokenError::InvalidRequest)?)
                .map_err(|_| TokenError::InvalidRequest)?;
            let verifier = request
                .code_verifier
                .clone()
                .map(SecretToken::new)
                .transpose()
                .map_err(|_| TokenError::InvalidRequest)?;
            service
                .exchange_from_origin(
                    CodeExchangeRequest {
                        code: &code,
                        client_id,
                        client_secret: secret.as_ref(),
                        redirect_uri: request.redirect_uri.as_deref(),
                        code_verifier: verifier.as_ref(),
                    },
                    origin,
                    context.id,
                    &context.budget,
                )
                .await?
        }
        "refresh_token" => {
            if request.code.is_some()
                || request.code_verifier.is_some()
                || request.redirect_uri.is_some()
            {
                return Err(TokenError::InvalidRequest);
            }
            let refresh = SecretToken::new(
                request
                    .refresh_token
                    .clone()
                    .ok_or(TokenError::InvalidRequest)?,
            )
            .map_err(|_| TokenError::InvalidRequest)?;
            service
                .refresh_from_origin(
                    RefreshRequest {
                        refresh_token: &refresh,
                        client_id,
                        client_secret: secret.as_ref(),
                    },
                    request.scope.as_deref(),
                    origin,
                    context.id,
                    &context.budget,
                )
                .await?
        }
        _ => return Err(TokenError::UnsupportedGrant),
    };
    let tokens = issued.tokens;
    Ok(Json(serde_json::json!({
        "access_token":tokens.access_token.expose_secret(),
        "id_token":tokens.id_token.as_ref().map(|token|token.expose_secret()),
        "token_type":"Bearer",
        "expires_in":tokens.claims.exp.saturating_sub(tokens.claims.iat),
        "refresh_token":tokens.refresh_token.expose_secret(),
        "scope":issued.scope,
    }))
    .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::CONTENT_TYPE;
    #[test]
    fn identity_token_client_authentication_rejects_conflicts_duplicates_and_malformed_basic() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        let body = b"grant_type=authorization_code&client_id=web&client_secret=secret";
        assert!(parse(&headers, body).is_ok());
        headers.insert(
            AUTHORIZATION,
            format!("Basic {}", STANDARD.encode("web:secret"))
                .parse()
                .unwrap(),
        );
        assert!(matches!(
            parse(&headers, body),
            Err(TokenError::InvalidRequest)
        ));
        assert!(parse(&headers, b"grant_type=authorization_code&client_id=web").is_ok());
        headers.insert(
            AUTHORIZATION,
            format!("basic {}", STANDARD.encode("web:secret"))
                .parse()
                .unwrap(),
        );
        assert!(parse(&headers, b"grant_type=authorization_code&client_id=web").is_ok());
        assert!(matches!(
            parse(&headers, b"grant_type=authorization_code&client_id=other"),
            Err(TokenError::InvalidRequest)
        ));
        for value in [
            "Basic !!",
            "Bearer opaque",
            &format!("Basic {}", STANDARD.encode("web:%ZZ")),
        ] {
            headers.insert(AUTHORIZATION, value.parse().unwrap());
            assert!(matches!(
                parse(&headers, b"grant_type=authorization_code"),
                Err(TokenError::InvalidClient)
            ));
        }
        headers.remove(AUTHORIZATION);
        for body in [
            b"grant_type=authorization_code&client_id=web&%63lient_id=web".as_slice(),
            b"grant_type=authorization_code&client_id=web&request_uri=https://example",
            b"grant_type=authorization_code&client_id=%ff",
        ] {
            assert!(parse(&headers, body).is_err());
        }
    }
}
