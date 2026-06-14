use crate::AppState;
use axum::Router;

pub mod cobotlet;
pub mod device;
#[cfg(feature = "easytier")]
pub mod easytier_config;
pub mod hello;
pub mod remote;
pub mod task;
pub mod tool;

/// Routes that require a valid JWT (verified by cobotrix itself).
/// Apply `auth::access_middleware` on top of this router.
pub fn get_protected_routes() -> Router<AppState> {
    let router = Router::new()
        .merge(device::get_routes())
        .merge(task::get_routes())
        .merge(remote::route())
        .merge(cobotlet::route())
        .merge(tool::route())
        .merge(crate::mcp::route::route());
    #[cfg(feature = "easytier")]
    let router = router.merge(easytier_config::get_protected_routes());
    router
}

/// Routes that are always public (health/debug only).
pub fn get_public_routes() -> Router<AppState> {
    let router = hello::route();
    #[cfg(feature = "easytier")]
    let router = router.merge(easytier_config::get_public_routes());
    router
}
