use crate::AppState;
use crate::rbac::check_authority_abac;
use axum::Router;
use axum::routing::post;
#[cfg(feature = "openapi")]
use utoipa::OpenApi;

pub fn get_public_routes() -> Router<AppState> {
    Router::new().route("/rbac/abac", post(check_authority_abac))
}

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::rbac::check_authority_abac,
    ),
    tags ((name = "RBAC权限校验", description = "外部校验权限"),)
)]
pub struct RbacApi {}
