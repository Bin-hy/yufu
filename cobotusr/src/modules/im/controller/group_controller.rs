use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
};
use serde::Deserialize;

use crate::{
    AppState,
    entity::user,
    modules::im::{
        helper::member::normalize_member_ids,
        model::{
            conversation, group,
            protocol::{
                AddGroupMembersResponse, ApiResponse, LeaveGroupResponse,
                RemoveGroupMemberResponse, TransferGroupOwnerResponse,
            },
        },
    },
};

#[derive(Debug, Deserialize)]
pub struct AddGroupMembersByConversationRequest {
    pub member_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TransferGroupOwnerByConversationRequest {
    pub new_owner_id: i64,
}

pub async fn add_members_by_conversation_id(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
    Json(req): Json<AddGroupMembersByConversationRequest>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(add_members_impl(
        state,
        current_user,
        conversation_id,
        req.member_ids,
    )))
    .await
}

async fn add_members_impl(
    state: AppState,
    current_user: user::Model,
    conversation_id: i64,
    member_ids: Vec<i64>,
) -> ApiResponse<AddGroupMembersResponse> {
    if member_ids.is_empty() {
        return super::bad_request::<AddGroupMembersResponse>("member_ids cannot be empty");
    }

    let db = &state.database.connection;
    let operator_id = i64::from(current_user.id);
    let _group = match super::load_group_owned_by::<AddGroupMembersResponse>(
        db,
        conversation_id,
        operator_id,
        "only group owner can add members",
    )
    .await
    {
        Ok(v) => v,
        Err(response) => return response,
    };

    let mut target_ids = normalize_member_ids(member_ids);
    target_ids.retain(|id| *id != operator_id);
    if target_ids.is_empty() {
        return super::bad_request::<AddGroupMembersResponse>("member_ids cannot be empty");
    }

    let existing = match conversation::list_member_ids(db, conversation_id).await {
        Ok(v) => v,
        Err(e) => {
            return super::bad_request::<AddGroupMembersResponse>(format!(
                "query members failed: {e}"
            ));
        }
    };

    let existing_set = existing
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let mut added_member_ids = Vec::new();
    let mut skipped_member_ids = Vec::new();
    for uid in target_ids {
        if existing_set.contains(&uid) {
            skipped_member_ids.push(uid);
        } else {
            added_member_ids.push(uid);
        }
    }

    if !added_member_ids.is_empty() {
        if let Err(e) = conversation::ensure_conversation_members(
            db,
            conversation_id,
            &added_member_ids,
            conversation::CONVERSATION_MEMBER_ROLE_MEMBER,
        )
        .await
        {
            return super::bad_request::<AddGroupMembersResponse>(format!(
                "add group members failed: {e}"
            ));
        }

        if let Err(e) =
            conversation::ensure_user_conversations(db, conversation_id, &added_member_ids, 0, None)
                .await
        {
            return super::bad_request::<AddGroupMembersResponse>(format!(
                "create user conversation states failed: {e}"
            ));
        }
    }

    ApiResponse::ok(AddGroupMembersResponse {
        conversation_id,
        added_member_ids,
        skipped_member_ids,
    })
}

pub async fn remove_member_by_conversation_id(
    State(state): State<AppState>,
    current_user: user::Model,
    Path((conversation_id, user_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(remove_member_impl(
        state,
        current_user,
        conversation_id,
        user_id,
    )))
    .await
}

async fn remove_member_impl(
    state: AppState,
    current_user: user::Model,
    conversation_id: i64,
    user_id: i64,
) -> ApiResponse<RemoveGroupMemberResponse> {
    if user_id <= 0 {
        return super::bad_request::<RemoveGroupMemberResponse>("user_id must be positive");
    }

    let db = &state.database.connection;
    let operator_id = i64::from(current_user.id);
    let group = match super::load_group_owned_by::<RemoveGroupMemberResponse>(
        db,
        conversation_id,
        operator_id,
        "only group owner can remove members",
    )
    .await
    {
        Ok(v) => v,
        Err(response) => return response,
    };
    if group.owner_id == user_id {
        return super::bad_request::<RemoveGroupMemberResponse>(
            "cannot remove owner, transfer ownership first",
        );
    }

    let removed =
        match conversation::remove_user_from_conversation(db, conversation_id, user_id).await {
            Ok(v) => v,
            Err(e) => {
                return super::bad_request::<RemoveGroupMemberResponse>(format!(
                    "remove member failed: {e}"
                ));
            }
        };

    ApiResponse::ok(RemoveGroupMemberResponse {
        conversation_id,
        user_id,
        removed,
    })
}

pub async fn leave_by_conversation_id(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(leave_impl(
        state,
        current_user,
        conversation_id,
    )))
    .await
}

async fn leave_impl(
    state: AppState,
    current_user: user::Model,
    conversation_id: i64,
) -> ApiResponse<LeaveGroupResponse> {
    let db = &state.database.connection;
    let user_id = i64::from(current_user.id);
    let group = match super::load_group::<LeaveGroupResponse>(db, conversation_id).await {
        Ok(v) => v,
        Err(response) => return response,
    };
    if group.owner_id == user_id {
        return super::bad_request::<LeaveGroupResponse>(
            "owner cannot leave group before transferring ownership",
        );
    }

    let is_member = match conversation::is_user_in_conversation(db, user_id, conversation_id).await
    {
        Ok(v) => v,
        Err(e) => {
            return super::bad_request::<LeaveGroupResponse>(format!("query member failed: {e}"));
        }
    };
    if !is_member {
        return super::bad_request::<LeaveGroupResponse>("user is not a group member");
    }

    if let Err(e) = conversation::remove_user_from_conversation(db, conversation_id, user_id).await
    {
        return super::bad_request::<LeaveGroupResponse>(format!("leave group failed: {e}"));
    }

    ApiResponse::ok(LeaveGroupResponse {
        conversation_id,
        user_id,
    })
}

pub async fn transfer_owner_by_conversation_id(
    State(state): State<AppState>,
    current_user: user::Model,
    Path(conversation_id): Path<i64>,
    Json(req): Json<TransferGroupOwnerByConversationRequest>,
) -> impl IntoResponse {
    super::join_response(tokio::spawn(transfer_owner_impl(
        state,
        current_user,
        conversation_id,
        req.new_owner_id,
    )))
    .await
}

async fn transfer_owner_impl(
    state: AppState,
    current_user: user::Model,
    conversation_id: i64,
    new_owner_id: i64,
) -> ApiResponse<TransferGroupOwnerResponse> {
    if new_owner_id <= 0 {
        return super::bad_request::<TransferGroupOwnerResponse>("new_owner_id must be positive");
    }

    let db = &state.database.connection;
    let operator_id = i64::from(current_user.id);
    let group = match super::load_group_owned_by::<TransferGroupOwnerResponse>(
        db,
        conversation_id,
        operator_id,
        "only current owner can transfer ownership",
    )
    .await
    {
        Ok(v) => v,
        Err(response) => return response,
    };
    if new_owner_id == operator_id {
        return super::bad_request::<TransferGroupOwnerResponse>(
            "new_owner_id cannot equal current owner",
        );
    }

    let is_member =
        match conversation::is_user_in_conversation(db, new_owner_id, conversation_id).await {
            Ok(v) => v,
            Err(e) => {
                return super::bad_request::<TransferGroupOwnerResponse>(format!(
                    "query new owner member failed: {e}"
                ));
            }
        };
    if !is_member {
        return super::bad_request::<TransferGroupOwnerResponse>(
            "new owner must be a group member",
        );
    }

    if let Err(e) = conversation::set_conversation_member_role(
        db,
        conversation_id,
        operator_id,
        conversation::CONVERSATION_MEMBER_ROLE_MEMBER,
    )
    .await
    {
        return super::bad_request::<TransferGroupOwnerResponse>(format!(
            "update old owner member role failed: {e}"
        ));
    }

    if let Err(e) = conversation::set_conversation_member_role(
        db,
        conversation_id,
        new_owner_id,
        conversation::CONVERSATION_MEMBER_ROLE_OWNER,
    )
    .await
    {
        return super::bad_request::<TransferGroupOwnerResponse>(format!(
            "update new owner member role failed: {e}"
        ));
    }

    if let Err(e) = group::set_owner(db, group, new_owner_id).await {
        return super::bad_request::<TransferGroupOwnerResponse>(format!(
            "transfer owner failed: {e}"
        ));
    }

    ApiResponse::ok(TransferGroupOwnerResponse {
        conversation_id,
        old_owner_id: operator_id,
        new_owner_id,
    })
}
