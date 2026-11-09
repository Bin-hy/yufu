use super::identity_browser::IdentityRequest;
use crate::{AppState, domain::identity::error::IdentityError};
use auth::security::identity::SecretToken;
use axum::{
    Extension, Json,
    extract::{RawQuery, State},
    response::{IntoResponse, Response},
};
use http::{
    HeaderMap, StatusCode,
    header::{AUTHORIZATION, WWW_AUTHENTICATE},
};

pub(crate) async fn userinfo(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let result = async {
        if query.as_ref().is_some_and(|q| !q.is_empty()) {
            return Err(IdentityError::InvalidInput);
        }
        let header =
            crate::service::security::browser::single_header(&headers, AUTHORIZATION.as_str())
                .map_err(|_| IdentityError::AccessDenied)?
                .ok_or(IdentityError::AccessDenied)?;
        let (scheme, value) = header.split_once(' ').ok_or(IdentityError::AccessDenied)?;
        if !scheme.eq_ignore_ascii_case("Bearer") {
            return Err(IdentityError::AccessDenied);
        }
        let token = SecretToken::new(value.to_owned()).map_err(|_| IdentityError::AccessDenied)?;
        super::identity_token::service(&state)?
            .userinfo_from_origin(
                &token,
                state.id_token_signer().as_ref(),
                crate::service::security::browser::single_header(&headers, "origin")?,
                &context.budget,
            )
            .await
    }
    .await;
    match result {
        Ok(profile) => Json(profile).into_response(),
        Err(
            IdentityError::AccessDenied | IdentityError::Unauthenticated | IdentityError::Forbidden,
        ) => {
            let mut response = (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error":"invalid_token","request_id":context.id})),
            )
                .into_response();
            response.headers_mut().insert(
                WWW_AUTHENTICATE,
                http::HeaderValue::from_static("Bearer error=\"invalid_token\""),
            );
            response
        }
        Err(error) => crate::error::AppError::identity(error, context.id).into_response(),
    }
}
