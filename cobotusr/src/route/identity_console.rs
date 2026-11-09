use super::{
    developer::{ManagementResponse, response},
    identity_browser::{IdentityRequest, sessions},
};
use crate::{
    AppState,
    domain::identity::{
        console::ConsoleKind, error::IdentityError, session::IdentityRevocationScope,
    },
    service::identity::{browser, console::ConsoleService, repository::PgIdentityRepository},
};
use axum::{
    Extension, Router,
    extract::{
        Path, Query, State,
        rejection::{PathRejection, QueryRejection},
    },
    routing::{get, post},
};
use http::HeaderMap;
use std::sync::Arc;
use uuid::Uuid;

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/account/apps", get(apps))
        .route("/account/consents", get(consents))
        .route("/account/consents/{id}/revoke", post(withdraw))
        .route("/account/sessions", get(centers))
        .route("/account/rp-sessions", get(rps))
        .route("/account/rp-sessions/{id}/revoke", post(revoke_rp))
        .route("/admin/accounts", get(accounts))
        .route("/admin/audit-events", get(audit))
        .route("/admin/logout-deliveries", get(deliveries))
        .route("/admin/logout-deliveries/{id}/retry", post(retry))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    after: Option<String>,
    #[serde(default = "default_limit")]
    limit: u32,
}
fn default_limit() -> u32 {
    25
}
fn console(state: &AppState) -> Result<ConsoleService, IdentityError> {
    let clock = state.security_clock()?;
    Ok(ConsoleService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
    ))
}
async fn list(
    state: AppState,
    context: IdentityRequest,
    headers: HeaderMap,
    query: Result<Query<Page>, QueryRejection>,
    kind: ConsoleKind,
) -> ManagementResponse {
    let result = async {
        let Query(page) = query.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        console(&state)?
            .list(
                &secret,
                kind,
                page.after.as_deref(),
                page.limit,
                &context.budget,
            )
            .await
    }
    .await;
    response(result, context.id)
}
macro_rules! list_route {
    ($name:ident,$kind:ident) => {
        async fn $name(
            State(state): State<AppState>,
            Extension(context): Extension<IdentityRequest>,
            headers: HeaderMap,
            query: Result<Query<Page>, QueryRejection>,
        ) -> ManagementResponse {
            list(state, context, headers, query, ConsoleKind::$kind).await
        }
    };
}
list_route!(apps, Applications);
list_route!(consents, Consents);
list_route!(centers, Centers);
list_route!(rps, RelyingParties);
list_route!(accounts, Accounts);
list_route!(audit, Audit);
list_route!(deliveries, Deliveries);
async fn withdraw(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(
            &state.config,
            state.security_clock()?.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        console(&state)?
            .withdraw_consent(&evidence, id, context.id, &context.budget)
            .await?;
        Ok::<_, IdentityError>(serde_json::json!({"revoked":true}))
    }
    .await;
    response(result, context.id)
}
async fn revoke_rp(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        browser::verify_write(
            &state.config,
            state.security_clock()?.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        sessions(&state)?
            .revoke_personal(
                &secret,
                IdentityRevocationScope::ApplicationLocal { rp_session_id: id },
                context.id,
                &context.budget,
            )
            .await?;
        Ok::<_, IdentityError>(serde_json::json!({"revoked":true}))
    }
    .await;
    response(result, context.id)
}
async fn retry(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<Uuid>, PathRejection>,
) -> ManagementResponse {
    let result = async {
        let Path(id) = path.map_err(|_| IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(
            &state.config,
            state.security_clock()?.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        console(&state)?
            .retry_delivery(
                &evidence,
                id,
                context.id,
                state.config.identity.logout_max_age_seconds,
                &context.budget,
            )
            .await?;
        Ok::<_, IdentityError>(serde_json::json!({"queued":true}))
    }
    .await;
    response(result, context.id)
}
