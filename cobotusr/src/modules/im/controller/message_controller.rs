use axum::{
    extract::{Json, Path, Query, State},
    response::IntoResponse,
};
use chrono::Utc;
use sea_orm::{EntityTrait, TransactionTrait};

use crate::{
    AppState,
    entity::{message as message_entity, user},
    modules::im::{
        helper::message_payload::merge_client_msg_id,
        model::{
            conversation,
            enums::ConversationType,
            message as message_model,
            message::MessageModel,
            protocol::{
                ApiResponse, DeleteMessageRequest, DeleteMessageResponse,
                GetConversationMessagesQuery, GetMessagesResponse, RecallMessageRequest,
                RecallMessageResponse, SendConversationMessageRequest,
                SendConversationMessageResponse, normalize_message_payload, parse_message_type,
            },
        },
        service::ImEvent,
    },
};

pub async fn get_messages_by_conversation_id(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
    Query(query): Query<GetConversationMessagesQuery>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(get_messages_impl(
        state,
        current_user,
        conversation_id,
        query.before_id,
        query.limit,
    )))
    .await
}

async fn get_messages_impl(
    state: AppState,
    current_user: user::Model,
    conversation_id: i64,
    before_id: Option<i64>,
    limit: Option<u64>,
) -> ApiResponse<GetMessagesResponse> {
    let user_id = i64::from(current_user.id);
    if let Err(response) = super::load_member_conversation::<GetMessagesResponse>(
        &state.database.connection,
        user_id,
        conversation_id,
    )
    .await
    {
        return response;
    }

    let limit = super::normalize_limit(limit, 50, 200);

    let (items, has_more) = match message_model::list_history_messages(
        &state.database.connection,
        conversation_id,
        before_id,
        limit,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return super::internal_error::<GetMessagesResponse>(e.to_string());
        }
    };

    ApiResponse::ok(GetMessagesResponse {
        conversation_id,
        before_id,
        limit,
        has_more,
        items: items.into_iter().map(MessageModel::from).collect(),
    })
}

pub async fn send_conversation_message(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
    Json(req): Json<SendConversationMessageRequest>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(async move {
        let client_msg_id = req.client_msg_id.clone();

        let msg_type = match parse_message_type(req.msg_type) {
            Ok(v) => v,
            Err(e) => {
                return super::bad_request::<SendConversationMessageResponse>(e);
            }
        };

        let (content, extra) = match normalize_message_payload(msg_type, req.content, req.extra) {
            Ok(v) => v,
            Err(e) => {
                return super::bad_request::<SendConversationMessageResponse>(e);
            }
        };

        let db = &state.database.connection;
        let sender_id = i64::from(current_user.id);
        let conv = match super::load_member_conversation::<SendConversationMessageResponse>(
            db,
            sender_id,
            conversation_id,
        )
        .await
        {
            Ok(v) => v,
            Err(response) => return response,
        };

        if let Some(client_msg_id_value) = client_msg_id.as_deref() {
            match message_model::find_message_by_client_msg_id(
                db,
                conversation_id,
                sender_id,
                client_msg_id_value,
            )
            .await
            {
                Ok(Some(existing)) => {
                    return ApiResponse::ok(SendConversationMessageResponse {
                        message_id: existing.id,
                        conversation_id,
                        msg_type: existing.msg_type,
                        client_msg_id,
                        created_at: existing.created_at.with_timezone(&Utc),
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    return super::bad_request::<SendConversationMessageResponse>(format!(
                        "query duplicate message failed: {e}"
                    ));
                }
            }
        }

        let extra = merge_client_msg_id(extra, client_msg_id.clone());
        let saved = match message_model::insert_message(
            db,
            conversation_id,
            sender_id,
            msg_type,
            content.clone(),
            extra.clone(),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                return super::bad_request::<SendConversationMessageResponse>(format!(
                    "insert message failed: {e}"
                ));
            }
        };

        let created_at = saved.created_at.with_timezone(&Utc);
        match conv.conversation_type {
            ConversationType::Single => {
                let member_ids = match conversation::list_member_ids(db, conversation_id).await {
                    Ok(v) => v,
                    Err(e) => {
                        return super::bad_request::<SendConversationMessageResponse>(format!(
                            "list members failed: {e}"
                        ));
                    }
                };
                let Some(receiver_id) = member_ids.into_iter().find(|id| *id != sender_id) else {
                    return super::bad_request::<SendConversationMessageResponse>(
                        "single chat receiver not found",
                    );
                };

                let _ = conversation::touch_private_conversations_on_send(
                    db,
                    conversation_id,
                    sender_id,
                    receiver_id,
                    saved.id,
                )
                .await;

                let _ = state.im_events.send(ImEvent::PrivateChat {
                    message_id: saved.id,
                    conversation_id,
                    from: sender_id,
                    to: receiver_id,
                    msg_type,
                    content,
                    extra,
                    created_at,
                });
            }
            ConversationType::Group => {
                let member_ids = match conversation::touch_group_conversation_on_send(
                    db,
                    conversation_id,
                    sender_id,
                    saved.id,
                )
                .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        return super::bad_request::<SendConversationMessageResponse>(format!(
                            "update group unread failed: {e}"
                        ));
                    }
                };

                let _ = state.im_events.send(ImEvent::GroupChat {
                    message_id: saved.id,
                    conversation_id,
                    from: sender_id,
                    targets: member_ids,
                    msg_type,
                    content,
                    extra,
                    created_at,
                });
            }
        }

        ApiResponse::ok(SendConversationMessageResponse {
            message_id: saved.id,
            conversation_id,
            msg_type,
            client_msg_id,
            created_at,
        })
    }))
    .await
}

pub async fn recall_message_by_conversation_id(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
    Json(req): Json<RecallMessageRequest>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(recall_message_impl(
        state,
        current_user,
        req,
        Some(conversation_id),
    )))
    .await
}

async fn recall_message_impl(
    state: AppState,
    current_user: user::Model,
    req: RecallMessageRequest,
    expected_conversation_id: Option<i64>,
) -> ApiResponse<RecallMessageResponse> {
    let db = &state.database.connection;
    let operator_id = i64::from(current_user.id);
    let original = match super::load_owned_message::<RecallMessageResponse, _>(
        db,
        operator_id,
        req.message_id,
        expected_conversation_id,
        "only sender can recall message",
    )
    .await
    {
        Ok(v) => v,
        Err(response) => return response,
    };

    let recalled_model = if original.is_recalled {
        original
    } else {
        match message_model::mark_message_recalled(db, original, Utc::now().fixed_offset()).await {
            Ok(v) => v,
            Err(e) => {
                return super::bad_request::<RecallMessageResponse>(format!(
                    "mark recalled failed: {e}"
                ));
            }
        }
    };

    let recalled_at = recalled_model
        .recalled_at
        .map(|v| v.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let member_ids = match conversation::list_member_ids(db, recalled_model.conversation_id).await {
        Ok(v) => v,
        Err(e) => {
            return super::bad_request::<RecallMessageResponse>(format!(
                "list members failed: {e}"
            ));
        }
    };

    let _ = state.im_events.send(ImEvent::MessageRecall {
        targets: member_ids,
        message_id: recalled_model.id,
        conversation_id: recalled_model.conversation_id,
        operator_id,
        recalled_at,
    });

    ApiResponse::ok(RecallMessageResponse {
        message_id: recalled_model.id,
        conversation_id: recalled_model.conversation_id,
        recalled_at,
    })
}

pub async fn delete_message_by_conversation_id(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
    Json(req): Json<DeleteMessageRequest>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(delete_message_impl(
        state,
        current_user,
        req,
        Some(conversation_id),
    )))
    .await
}

async fn delete_message_impl(
    state: AppState,
    current_user: user::Model,
    req: DeleteMessageRequest,
    expected_conversation_id: Option<i64>,
) -> ApiResponse<DeleteMessageResponse> {
    let db = &state.database.connection;
    let operator_id = i64::from(current_user.id);
    let txn = match db.begin().await {
        Ok(v) => v,
        Err(e) => {
            return super::bad_request::<DeleteMessageResponse>(format!(
                "begin delete transaction failed: {e}"
            ));
        }
    };

    let original = match super::load_owned_message::<DeleteMessageResponse, _>(
        &txn,
        operator_id,
        req.message_id,
        expected_conversation_id,
        "only sender can delete message",
    )
    .await
    {
        Ok(v) => v,
        Err(response) => return response,
    };

    let deleted_at = Utc::now();
    if let Err(e) = message_entity::Entity::delete_by_id(req.message_id)
        .exec(&txn)
        .await
    {
        return super::bad_request::<DeleteMessageResponse>(format!("delete message failed: {e}"));
    }

    if let Err(e) = txn.commit().await {
        return super::bad_request::<DeleteMessageResponse>(format!(
            "commit delete transaction failed: {e}"
        ));
    }

    let member_ids = match conversation::list_member_ids(db, original.conversation_id).await {
        Ok(v) => v,
        Err(e) => {
            return super::bad_request::<DeleteMessageResponse>(format!(
                "list members failed: {e}"
            ));
        }
    };

    let _ = state.im_events.send(ImEvent::MessageDelete {
        targets: member_ids,
        message_id: req.message_id,
        conversation_id: original.conversation_id,
        operator_id,
        deleted_at,
    });

    ApiResponse::ok(DeleteMessageResponse {
        message_id: req.message_id,
        conversation_id: original.conversation_id,
        deleted_at,
    })
}
