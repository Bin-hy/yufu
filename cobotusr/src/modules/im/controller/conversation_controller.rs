use axum::{
    extract::{Json, Path, Query, State},
    response::IntoResponse,
};
use chrono::Utc;

use crate::{
    AppState,
    entity::user,
    modules::im::{
        helper::{conversation_id::private_conversation_id, member::normalize_member_ids},
        model::{
            conversation,
            enums::ConversationType,
            group,
            protocol::{
                ApiResponse, ClearUnreadResponse, ConversationDetailResponse,
                ConversationListQuery, ConversationListResponse, ConversationMembersResponse,
                ConversationSummary, CreateConversationRequest, CreateConversationResponse,
                DeleteConversationResponse, parse_conversation_type,
            },
        },
        service::ImEvent,
    },
};

pub async fn create_conversation(
    State(state): State<AppState>,
    current_user: user::Model,
    Json(req): Json<CreateConversationRequest>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(async move {
        let conversation_type = match parse_conversation_type(req.conversation_type) {
            Ok(v) => v,
            Err(e) => {
                return super::bad_request::<CreateConversationResponse>(e);
            }
        };

        if req.member_ids.is_empty() {
            return super::bad_request::<CreateConversationResponse>("member_ids cannot be empty");
        }

        let db = &state.database.connection;
        let creator_id = i64::from(current_user.id);
        match conversation_type {
            ConversationType::Single => {
                let mut member_ids = normalize_member_ids(req.member_ids);
                member_ids.retain(|id| *id != creator_id);

                if member_ids.len() != 1 {
                    return super::bad_request::<CreateConversationResponse>(
                        "single chat must provide exactly one target member_id",
                    );
                }

                let peer_id = member_ids[0];
                let conversation_id = private_conversation_id(creator_id, peer_id);
                if let Err(e) = conversation::ensure_private_conversation(db, conversation_id).await
                {
                    return super::bad_request::<CreateConversationResponse>(format!(
                        "ensure private conversation failed: {e}"
                    ));
                }

                let participants = vec![creator_id, peer_id];
                if let Err(e) = conversation::ensure_conversation_members(
                    db,
                    conversation_id,
                    &participants,
                    conversation::CONVERSATION_MEMBER_ROLE_MEMBER,
                )
                .await
                {
                    return super::bad_request::<CreateConversationResponse>(format!(
                        "create conversation members failed: {e}"
                    ));
                }

                if let Err(e) = conversation::ensure_user_conversations(
                    db,
                    conversation_id,
                    &participants,
                    0,
                    None,
                )
                .await
                {
                    return super::bad_request::<CreateConversationResponse>(format!(
                        "create user conversation rows failed: {e}"
                    ));
                }

                ApiResponse::ok(CreateConversationResponse {
                    conversation_id,
                    conversation_type: ConversationType::Single,
                    title: None,
                    member_ids: participants,
                })
            }
            ConversationType::Group => {
                let mut member_ids = normalize_member_ids(req.member_ids);
                if !member_ids.contains(&creator_id) {
                    member_ids.push(creator_id);
                }
                member_ids.sort_unstable();
                member_ids.dedup();

                if member_ids.len() < 2 {
                    return super::bad_request::<CreateConversationResponse>(
                        "group chat must have at least 2 members",
                    );
                }

                let title = req
                    .title
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| "Group Chat".to_string());

                let conv = match conversation::create_conversation(
                    db,
                    ConversationType::Group,
                    Some(title.clone()),
                )
                .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        return super::bad_request::<CreateConversationResponse>(format!(
                            "create conversation failed: {e}"
                        ));
                    }
                };

                if let Err(e) =
                    group::create_group(db, conv.id, title.clone(), creator_id, None).await
                {
                    return super::bad_request::<CreateConversationResponse>(format!(
                        "create group failed: {e}"
                    ));
                }

                if let Err(e) = conversation::ensure_conversation_members(
                    db,
                    conv.id,
                    &member_ids,
                    conversation::CONVERSATION_MEMBER_ROLE_MEMBER,
                )
                .await
                {
                    return super::bad_request::<CreateConversationResponse>(format!(
                        "create group members failed: {e}"
                    ));
                }
                if let Err(e) = conversation::set_conversation_member_role(
                    db,
                    conv.id,
                    creator_id,
                    conversation::CONVERSATION_MEMBER_ROLE_OWNER,
                )
                .await
                {
                    return super::bad_request::<CreateConversationResponse>(format!(
                        "set group owner member role failed: {e}"
                    ));
                }

                if let Err(e) =
                    conversation::ensure_user_conversations(db, conv.id, &member_ids, 0, None).await
                {
                    return super::bad_request::<CreateConversationResponse>(format!(
                        "create user conversation states failed: {e}"
                    ));
                }

                ApiResponse::ok(CreateConversationResponse {
                    conversation_id: conv.id,
                    conversation_type: ConversationType::Group,
                    title: Some(title),
                    member_ids,
                })
            }
        }
    }))
    .await
}

pub async fn get_conversations(
    State(state): State<AppState>,
    current_user: user::Model,
    Query(query): Query<ConversationListQuery>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(async move {
        let limit = super::normalize_limit(query.limit, 50, 200);
        let user_id = i64::from(current_user.id);

        let items =
            match conversation::list_user_conversations(&state.database.connection, user_id, limit)
                .await
            {
                Ok(v) => v
                    .into_iter()
                    .map(|item| ConversationSummary {
                        conversation_id: item.conversation_id,
                        conversation_type: item.conversation_type,
                        title: item.title,
                        is_pinned: item.is_pinned,
                        unread_count: item.unread_count,
                        last_message_preview: item.last_message_preview,
                    })
                    .collect(),
                Err(e) => {
                    return super::internal_error::<ConversationListResponse>(e.to_string());
                }
            };

        ApiResponse::ok(ConversationListResponse { limit, items })
    }))
    .await
}

pub async fn get_conversation(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(async move {
        let db = &state.database.connection;
        let user_id = i64::from(current_user.id);
        let conv = match super::load_member_conversation::<ConversationDetailResponse>(
            db,
            user_id,
            conversation_id,
        )
        .await
        {
            Ok(v) => v,
            Err(response) => return response,
        };

        let member_ids = match conversation::list_member_ids(db, conversation_id).await {
            Ok(v) => v,
            Err(e) => {
                return super::internal_error::<ConversationDetailResponse>(format!(
                    "list members failed: {e}"
                ));
            }
        };

        // Fetch owner_id for group conversations
        let owner_id = if conv.conversation_type == ConversationType::Group {
            match group::find_by_conversation_id(db, conversation_id).await {
                Ok(Some(g)) => Some(g.owner_id),
                Ok(None) => None,
                Err(_) => None,
            }
        } else {
            None
        };

        ApiResponse::ok(ConversationDetailResponse {
            conversation_id,
            conversation_type: conv.conversation_type,
            title: conv.title,
            member_ids,
            owner_id,
        })
    }))
    .await
}

pub async fn delete_conversation(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(async move {
        let db = &state.database.connection;
        let user_id = i64::from(current_user.id);
        let conv = match super::load_member_conversation::<DeleteConversationResponse>(
            db,
            user_id,
            conversation_id,
        )
        .await
        {
            Ok(v) => v,
            Err(response) => return response,
        };

        if conv.conversation_type == ConversationType::Group {
            match group::find_by_conversation_id(db, conversation_id).await {
                Ok(Some(group)) if group.owner_id == user_id => {
                    return super::bad_request::<DeleteConversationResponse>(
                        "owner cannot delete group conversation before transferring ownership",
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    return super::internal_error::<DeleteConversationResponse>(format!(
                        "query group failed: {e}"
                    ));
                }
            }
        }

        let deleted =
            match conversation::remove_user_from_conversation(db, conversation_id, user_id).await {
                Ok(v) => v,
                Err(e) => {
                    return super::internal_error::<DeleteConversationResponse>(format!(
                        "delete conversation failed: {e}"
                    ));
                }
            };

        ApiResponse::ok(DeleteConversationResponse {
            conversation_id,
            deleted,
        })
    }))
    .await
}

pub async fn list_conversation_members(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(async move {
        let db = &state.database.connection;
        let user_id = i64::from(current_user.id);
        let conv = match super::load_member_conversation::<ConversationMembersResponse>(
            db,
            user_id,
            conversation_id,
        )
        .await
        {
            Ok(v) => v,
            Err(response) => return response,
        };

        let member_ids = match conversation::list_member_ids(db, conversation_id).await {
            Ok(v) => v,
            Err(e) => {
                return super::internal_error::<ConversationMembersResponse>(format!(
                    "list members failed: {e}"
                ));
            }
        };

        // Fetch owner_id for group conversations
        let owner_id = if conv.conversation_type == ConversationType::Group {
            match group::find_by_conversation_id(db, conversation_id).await {
                Ok(Some(g)) => Some(g.owner_id),
                Ok(None) => None,
                Err(_) => None,
            }
        } else {
            None
        };

        ApiResponse::ok(ConversationMembersResponse {
            conversation_id,
            member_ids,
            owner_id,
        })
    }))
    .await
}

pub async fn clear_unread_by_conversation_id(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(clear_unread_impl(
        state,
        current_user,
        conversation_id,
    )))
    .await
}

async fn clear_unread_impl(
    state: AppState,
    current_user: user::Model,
    conversation_id: i64,
) -> ApiResponse<ClearUnreadResponse> {
    let user_id = i64::from(current_user.id);
    if let Err(response) = super::load_member_conversation::<ClearUnreadResponse>(
        &state.database.connection,
        user_id,
        conversation_id,
    )
    .await
    {
        return response;
    }

    let (last_read_message_id, unread_count) = match conversation::clear_unread(
        &state.database.connection,
        user_id,
        conversation_id,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return super::internal_error::<ClearUnreadResponse>(e.to_string());
        }
    };

    if let Ok(member_ids) =
        conversation::list_member_ids(&state.database.connection, conversation_id).await
    {
        let targets = member_ids
            .into_iter()
            .filter(|uid| *uid != user_id)
            .collect::<Vec<_>>();
        if !targets.is_empty() {
            let _ = state.im_events.send(ImEvent::ConversationRead {
                targets,
                conversation_id,
                reader_id: user_id,
                last_read_message_id,
                read_at: Utc::now(),
            });
        }
    }

    ApiResponse::ok(ClearUnreadResponse {
        conversation_id,
        unread_count,
        last_read_message_id,
    })
}
