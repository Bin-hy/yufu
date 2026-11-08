use crate::AppState;
use crate::modules::im;
use axum::Router;

pub mod application;
pub mod auth;
pub mod authority;
pub mod enforce;
pub mod mcp;
pub mod oidc;
pub mod oidc_admin;
pub mod permission;
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
        .nest("/im", im::router::init_im_router())
}

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
        .merge(user::get_public_routes())
        .merge(auth::get_public_routes())
        .merge(rbac::get_public_routes())
        .merge(sync_permissions::get_public_routes())
}

/// These mutations authenticate and authorize through the local primary transaction.
pub fn get_revocation_routes() -> Router<AppState> {
    Router::new()
        .merge(auth::get_revocation_routes())
        .merge(oidc::get_revocation_routes())
        .merge(mcp::get_management_routes())
}

/// Service-to-service routes: JWT-verified, but deliberately WITHOUT the
/// user-load / URI-RBAC / recording middleware, so a misconfigured authority
/// row or deleted user cannot produce non-JSON errors for machine callers
/// (cobotsrv's MCP revocation client fails open on those).
pub fn get_service_routes() -> Router<AppState> {
    Router::new().merge(mcp::get_service_routes())
}

pub fn get_no_prefix_routes() -> Router<AppState> {
    Router::new().merge(oidc::get_public_routes())
}
