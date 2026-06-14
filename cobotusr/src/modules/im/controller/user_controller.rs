use axum::{
    extract::{Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashSet;

use crate::AppState;
use crate::entity::user;
use crate::modules::im::model::protocol::ApiResponse;

#[derive(Debug, Deserialize)]
pub struct ImUsersQuery {
    pub ids: Option<String>,
    pub all: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ImUserInfo {
    pub id: i64,
    pub username: String,
    pub nickname: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ImUsersResponse {
    pub users: Vec<ImUserInfo>,
}

pub async fn get_users_by_ids(
    State(state): State<AppState>,
    Query(query): Query<ImUsersQuery>,
) -> impl IntoResponse {
    let db = &state.database.connection;

    let models = if query.all == Some(true) {
        match user::Entity::find().all(db).await {
            Ok(v) => v,
            Err(e) => return super::internal_error::<ImUsersResponse>(format!("query users failed: {e}")),
        }
    } else {
        let ids_str = match query.ids.as_deref() {
            Some(s) if !s.trim().is_empty() => s,
            _ => return super::bad_request::<ImUsersResponse>("ids parameter is required"),
        };

        let id_set: HashSet<i64> = ids_str
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .filter(|id| *id > 0)
            .collect();

        if id_set.is_empty() {
            return super::bad_request::<ImUsersResponse>("no valid user IDs provided");
        }

        let ids: Vec<i64> = id_set.into_iter().collect();

        match user::Entity::find()
            .filter(user::Column::Id.is_in(ids))
            .all(db)
            .await
        {
            Ok(v) => v,
            Err(e) => return super::internal_error::<ImUsersResponse>(format!("query users failed: {e}")),
        }
    };

    let users: Vec<ImUserInfo> = models
        .into_iter()
        .map(|u| ImUserInfo {
            id: i64::from(u.id),
            username: u.username,
            nickname: u.nickname,
        })
        .collect();

    ApiResponse::ok(ImUsersResponse { users })
}
