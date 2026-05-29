use axum::{
    extract::{State, WebSocketUpgrade},
    response::IntoResponse,
};

use crate::{AppState, entity::user, modules::im::service::handle_socket};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    current_user: user::Model,
) -> impl IntoResponse {
    let manager = state.ws_handle.clone();
    let db = state.database.connection.clone();
    let user_id = i64::from(current_user.id);

    ws.on_upgrade(move |socket| async move {
        handle_socket(socket, user_id, manager, db).await;
    })
}
