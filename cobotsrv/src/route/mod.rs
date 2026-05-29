use crate::AppState;
use axum::Router;

pub mod cobotlet;
pub mod device;
pub mod hello;
pub mod remote;
pub mod task;
pub mod tool;

/// Routes that require a valid JWT (verified by cobotrix itself).
/// Apply `auth::access_middleware` on top of this router.
pub fn get_protected_routes() -> Router<AppState> {
    Router::new()
        .merge(device::get_routes())
        .merge(task::get_routes())
        .merge(remote::route())
        .merge(cobotlet::route())
        .merge(tool::route())
}

/// Routes that are always public (health/debug only).
pub fn get_public_routes() -> Router<AppState> {
    hello::route()
}
