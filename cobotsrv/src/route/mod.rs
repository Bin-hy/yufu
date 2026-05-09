use crate::AppState;
use axum::Router;

pub mod application;
pub mod auth;
pub mod authority;
pub mod enforce;
pub mod oidc;
pub mod oidc_admin;
pub mod permission;
pub mod proxy;
pub mod rbac;
pub mod recording;
pub mod role;
pub mod role_permission;
pub mod sync_permissions;
pub mod user;
pub mod user_authority;
pub mod user_permission;

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .merge(user::get_auth_routes())
        .merge(auth::get_auth_routes())
        .merge(role::get_auth_routes())
        .merge(authority::get_auth_routes())
        .merge(recording::get_auth_routes())
        .merge(user_authority::get_auth_routes())
        .merge(oidc::get_auth_routes())
        .merge(oidc_admin::get_auth_routes())
        .merge(enforce::get_auth_routes())
        .merge(application::get_auth_routes())
        .merge(permission::get_auth_routes())
        .merge(role_permission::get_auth_routes())
        .merge(user_permission::get_auth_routes())
}

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
        .merge(user::get_public_routes())
        .merge(auth::get_public_routes())
        .merge(rbac::get_public_routes())
        .merge(sync_permissions::get_public_routes())
}

pub fn get_no_prefix_routes() -> Router<AppState> {
    Router::new().merge(oidc::get_public_routes())
}
