use super::{
    identity_browser::IdentityRequest,
    identity_token::{self, TokenError},
    oidc::{IntrospectRequest, RevokeRequest},
};
use crate::{
    AppState,
    service::identity::{
        protocol::ProtocolClientAuthentication,
        repository::{IdentityRepository, PgIdentityRepository},
    },
};
use auth::security::identity::SecretToken;
use axum::{
    Extension, Json,
    body::Bytes,
    extract::State,
    response::{IntoResponse, Response},
};
use http::HeaderMap;

async fn status(
    state: AppState,
    context: IdentityRequest,
    headers: HeaderMap,
    body: Bytes,
    revoke: bool,
) -> Response {
    let result = async {
        let mut fields = identity_token::authenticated_form(
            &headers,
            &body,
            &["token", "token_type_hint", "client_id", "client_secret"],
        )?;
        let id = fields
            .remove("client_id")
            .ok_or(TokenError::InvalidClient)?;
        let secret = fields
            .remove("client_secret")
            .map(SecretToken::new)
            .transpose()
            .map_err(|_| TokenError::InvalidClient)?;
        let token = SecretToken::new(
            fields
                .remove("token")
                .filter(|v| !v.is_empty())
                .ok_or(TokenError::InvalidRequest)?,
        )
        .map_err(|_| TokenError::InvalidRequest)?;
        let clock = state
            .security_clock()
            .map_err(|e| TokenError::Backend(e.into()))?;
        let repo = PgIdentityRepository::new(state.database.connection.clone(), clock);
        let mut tx = repo.begin(&context.budget).await?;
        let client = tx
            .oidc_client(&id)
            .await
            .map_err(|e| TokenError::Backend(e.into()));
        tx.rollback()
            .await
            .map_err(|e| TokenError::Backend(e.into()))?;
        if let Some(client) = client?
            && client.management_state == "legacy_unmanaged"
        {
            if client.public_client && !revoke {
                return Err(TokenError::InvalidClient);
            }
            let secret = secret.as_ref().map(|s| s.expose_secret().to_owned());
            return if revoke {
                Ok(super::oidc::revoke_legacy(
                    State(state),
                    axum::extract::Form(RevokeRequest {
                        token: token.expose_secret().into(),
                        token_type_hint: fields.remove("token_type_hint").unwrap_or_default(),
                        client_id: Some(id),
                        client_secret: secret,
                    }),
                    &context.budget,
                )
                .await
                .into_response())
            } else {
                Ok(super::oidc::introspect_legacy(
                    State(state),
                    axum::extract::Form(IntrospectRequest {
                        token: token.expose_secret().into(),
                        token_type_hint: fields.remove("token_type_hint"),
                        client_id: Some(id),
                        client_secret: secret,
                    }),
                )
                .await
                .into_response())
            };
        }
        let service = identity_token::service(&state)?;
        let auth = ProtocolClientAuthentication {
            id: &id,
            secret: secret.as_ref(),
        };
        if revoke {
            service
                .revoke(
                    auth,
                    &token,
                    state.id_token_signer().as_ref(),
                    context.id,
                    &context.budget,
                )
                .await?;
            Ok(http::StatusCode::OK.into_response())
        } else {
            Ok(Json(
                service
                    .introspect(
                        auth,
                        &token,
                        state.id_token_signer().as_ref(),
                        &context.budget,
                    )
                    .await?,
            )
            .into_response())
        }
    }
    .await;
    result.unwrap_or_else(|error| identity_token::response_error(error, context.id))
}
pub(crate) async fn introspect(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    status(state, context, headers, body, false).await
}
pub(crate) async fn revoke(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    status(state, context, headers, body, true).await
}
