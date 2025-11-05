use crate::AppState;
use crate::rbac::check_authority;
use axum::Router;
use axum::routing::post;

pub fn get_public_routes() -> Router<AppState> {
    Router::new().route("/rbac", post(check_authority))
}
