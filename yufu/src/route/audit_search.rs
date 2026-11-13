use super::{
    developer::{ManagementResponse, response},
    identity_browser::IdentityRequest,
};
use crate::{
    AppState,
    domain::identity::{audit::AuditFilterInput, error::IdentityError},
    service::identity::{audit::AuditService, browser, repository::PgIdentityRepository},
};
use axum::{
    Extension, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use http::HeaderMap;
use std::sync::Arc;

pub fn get_routes() -> Router<AppState> {
    Router::new().route("/admin/audit-events/search", get(search))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    from: Option<String>,
    to: Option<String>,
    actor: Option<String>,
    action: Option<String>,
    object_kind: Option<String>,
    object_id: Option<String>,
    request_id: Option<uuid::Uuid>,
    application_id: Option<i32>,
    result: Option<String>,
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: u32,
}
fn default_limit() -> u32 {
    25
}
async fn search(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> ManagementResponse {
    let result = async {
        let Query(query) = query.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let clock = state.security_clock()?;
        AuditService::new(
            Arc::new(PgIdentityRepository::new(
                state.database.connection.clone(),
                clock.clone(),
            )),
            clock,
        )
        .search(
            &secret,
            AuditFilterInput {
                from: query.from,
                to: query.to,
                actor: query.actor,
                action: query.action,
                object_kind: query.object_kind,
                object_id: query.object_id,
                request_id: query.request_id,
                application_id: query.application_id,
                result: query.result,
            },
            query.cursor.as_deref(),
            query.limit,
            &context.budget,
        )
        .await
    }
    .await;
    response(result, context.id)
}
