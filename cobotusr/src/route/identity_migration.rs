use super::{
    developer::{ManagementResponse, applications, response, single_key},
    identity_browser::IdentityRequest,
};
use crate::{
    AppState,
    domain::identity::{error::IdentityError, migration::AdoptLegacyApplication},
    service::identity::browser,
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    routing::{get, post},
};
use http::HeaderMap;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    #[serde(default)]
    after: i32,
    #[serde(default = "limit")]
    limit: u32,
}
fn limit() -> u32 {
    20
}
pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/identity/migrations", get(list))
        .route("/admin/identity/migrations/scan", post(scan))
        .route("/admin/identity/migrations/{id}/adopt", post(adopt))
}
async fn list(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    page: Result<Query<Page>, QueryRejection>,
) -> ManagementResponse {
    let result = async {
        let Query(page) = page.map_err(|_| IdentityError::InvalidInput)?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        applications(&state)?
            .legacy_clients(&secret, page.after, page.limit, &context.budget)
            .await
    }
    .await;
    response(result, context.id)
}
async fn scan(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    body: Result<Json<Page>, JsonRejection>,
) -> ManagementResponse {
    let result = async {
        let Json(page) = body.map_err(|_| IdentityError::InvalidInput)?;
        let evidence = browser::verify_write(
            &state.config,
            state.security_clock()?.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        applications(&state)?
            .scan_legacy(
                &evidence,
                page.after,
                page.limit,
                context.id,
                &context.budget,
            )
            .await
    }
    .await;
    response(result, context.id)
}
async fn adopt(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<i32>, PathRejection>,
    body: Result<Json<AdoptLegacyApplication>, JsonRejection>,
) -> ManagementResponse {
    let result=async{
        let Path(id)=path.map_err(|_|IdentityError::InvalidInput)?;
        let Json(body)=body.map_err(|_|IdentityError::InvalidInput)?;
        if id!=body.oauth_client_id{return Err(IdentityError::InvalidInput);}
        let evidence=browser::verify_write(&state.config,state.security_clock()?.as_ref(),&headers,None,&context.budget)?;
        let receipt=applications(&state)?.adopt_legacy(&evidence,body,single_key(&headers)?,context.id,&context.budget).await?;
        let secrets:Vec<_>=receipt.newly_issued_secrets.into_iter().map(|(environment_id,secret)|serde_json::json!({"environment_id":environment_id,"client_secret":secret.into_response()})).collect();
        Ok::<_,IdentityError>(serde_json::json!({"operation_id":receipt.operation_id,"application_id":receipt.application_id,"management_mode":receipt.management_mode,"environments":receipt.environments,"newly_issued_secrets":secrets}))
    }.await;
    response(result, context.id)
}
