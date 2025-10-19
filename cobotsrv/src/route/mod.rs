use crate::AppState;
use axum::Router;

pub mod auth;
pub mod hello;
pub mod remote;
pub mod role;
pub mod user;

pub fn get_auth_routes() -> Router<AppState> {
    // handle route merge

    let routes = Router::new()
        .merge(user::get_auth_routes())
        .merge(auth::get_auth_routes());

    routes
}

pub fn get_public_routes() -> Router<AppState> {
    let routes = Router::new()
        .merge(user::get_public_routes())
        .merge(auth::get_public_routes());

    routes
}
