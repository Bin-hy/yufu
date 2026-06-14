use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, patch, post, put},
};

use crate::AppState;

use super::controller::{
    conversation_controller, group_controller, message_controller, outbox_controller,
    upload_controller, user_controller, websocket_controller,
};

const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

pub fn init_im_router() -> Router<AppState> {
    Router::new()
        .route("/users", get(user_controller::get_users_by_ids))
        .route("/ws", get(websocket_controller::ws_handler))
        .route(
            "/conversations",
            get(conversation_controller::get_conversations)
                .post(conversation_controller::create_conversation),
        )
        .route(
            "/conversations/{id}",
            get(conversation_controller::get_conversation)
                .delete(conversation_controller::delete_conversation),
        )
        .route(
            "/conversations/{id}/members",
            get(conversation_controller::list_conversation_members)
                .post(group_controller::add_members_by_conversation_id),
        )
        .route(
            "/conversations/{id}/members/me",
            delete(group_controller::leave_by_conversation_id),
        )
        .route(
            "/conversations/{id}/members/{user_id}",
            delete(group_controller::remove_member_by_conversation_id),
        )
        .route(
            "/conversations/{id}/owner",
            patch(group_controller::transfer_owner_by_conversation_id),
        )
        .route(
            "/conversations/{id}/read-state",
            put(conversation_controller::clear_unread_by_conversation_id),
        )
        .route(
            "/conversations/{id}/messages",
            get(message_controller::get_messages_by_conversation_id)
                .post(message_controller::send_conversation_message)
                .delete(message_controller::delete_message_by_conversation_id)
                .patch(message_controller::recall_message_by_conversation_id),
        )
        .route(
            "/files",
            get(upload_controller::get_file)
                .post(upload_controller::upload_file)
                .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES)),
        )
        .route(
            "/outbox-dispatches",
            post(outbox_controller::dispatch_outbox),
        )
}
