use crate::rbac::check_authority;
use crate::AppState;
use axum::routing::post;
use axum::Router;
#[cfg(feature = "openapi")]
use utoipa::OpenApi;

pub fn get_public_routes() -> Router<AppState> {
    Router::new().route("/rbac", post(check_authority))
}

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::rbac::check_authority,
    ),
    tags ((name = "RBAC权限校验", description = "外部校验权限"),)
)]
pub struct RbacApi {}
