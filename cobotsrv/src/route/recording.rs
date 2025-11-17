use crate::entity::recording::RecordingVO;
use crate::entity::{PageSelectQuery, PageSelectQueryResult, recording, user};
use crate::result::Result;
use crate::{AppState, IdType};
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
#[cfg(feature = "openapi")]
use utoipa::OpenApi;

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/recordings", get(get_recording_by_page))
        .route("/recordings/{user_id}", get(get_recording_by_user_id))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/recordings",
    params(PageSelectQuery),
    description = "分页获取所有用户所有操作记录",
    responses(
        (status = 200, description = "成功", body = PageSelectQueryResult<RecordingVO>),
    ),
))]
pub async fn get_recording_by_page(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<RecordingVO>>> {
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;
    let paginator = recording::Entity::find()
        .find_also_related(user::Entity)
        .order_by_desc(recording::Column::CreateTime)
        .paginate(db, page.page_size);

    let total_pages = paginator.num_pages().await?;
    let page_records: Vec<(recording::Model, Option<user::Model>)> =
        paginator.fetch_page(page.page_number - 1).await?;
    Ok(Json(PageSelectQueryResult::<RecordingVO> {
        page_size,
        page_number,
        total_pages,
        data_list: page_records.into_iter().map(RecordingVO::from).collect(),
    }))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/recordings/{user_id}",
    params(PageSelectQuery,
    ("user_id", description = "查询的用户Id")),
    description = "根据用户Id分页获取用户所有操作记录",
    responses(
        (status = 200, description = "成功", body = PageSelectQueryResult<RecordingVO>),
    ),
))]
pub async fn get_recording_by_user_id(
    State(state): State<AppState>,
    Path(user_id): Path<IdType>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<RecordingVO>>> {
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;
    let paginator = recording::Entity::find()
        .find_also_related(user::Entity)
        .filter(recording::Column::UserId.eq(user_id))
        .order_by_desc(recording::Column::CreateTime)
        .paginate(db, page.page_size);
    // 获取某个用户的所有记录
    let total_pages = paginator.num_pages().await?;
    let page_records: Vec<(recording::Model, Option<user::Model>)> =
        paginator.fetch_page(page.page_number - 1).await?;
    Ok(Json(PageSelectQueryResult::<RecordingVO> {
        page_size,
        page_number,
        total_pages,
        data_list: page_records.into_iter().map(RecordingVO::from).collect(),
    }))
}

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(paths(get_recording_by_user_id, get_recording_by_page))]
pub struct RecordingApi;
