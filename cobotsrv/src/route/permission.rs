use crate::entity::permission::{
    self, PermissionDeleteReq, PermissionInsertReq, PermissionPatchReq, PermissionVO,
};
use crate::entity::{PageSelectQuery, PageSelectQueryResult, application};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use crate::{AppState, IdType};
use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::routing::post;
use sea_orm::EntityTrait;
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct PermissionFilter {
    #[serde(default)]
    pub application_id: Option<IdType>,
}

pub fn get_auth_routes() -> Router<AppState> {
    Router::new().route(
        "/permission",
        post(add_permission)
            .get(get_page_permissions)
            .patch(patch_permission)
            .delete(delete_permission),
    )
}

pub async fn add_permission(
    State(state): State<AppState>,
    Json(insert_payload): Json<PermissionInsertReq>,
) -> Result<Json<PermissionVO>> {
    let db = &state.database.connection;
    // validate application exists
    if application::Entity::find_by_id(insert_payload.application_id)
        .one(db)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("应用不存在".to_string()));
    }
    let new_perm = permission::Entity::insert_one(db, insert_payload)
        .await
        .into_app_with_conflict_msg("权限名称已存在")?;
    Ok(Json(new_perm.into()))
}

pub async fn get_page_permissions(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
    Query(filter): Query<PermissionFilter>,
) -> Result<Json<PageSelectQueryResult<PermissionVO>>> {
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    let db = &state.database.connection;
    let (total_pages, records) = if let Some(app_id) = filter.application_id {
        let paginator = permission::Entity::find()
            .filter(permission::Column::ApplicationId.eq(app_id))
            .paginate(db, page.page_size);
        let total_pages = paginator.num_pages().await?;
        let records = paginator.fetch_page(page.page_number - 1).await?;
        (total_pages, records)
    } else {
        permission::Entity::paginate(db, page.page_size, page.page_number).await?
    };
    Ok(Json(PageSelectQueryResult {
        page_size: page.page_size,
        page_number: page.page_number,
        total_pages,
        data_list: records.into_iter().map(PermissionVO::from).collect(),
    }))
}

pub async fn patch_permission(
    State(state): State<AppState>,
    Json(patch_payload): Json<PermissionPatchReq>,
) -> Result<Json<PermissionVO>> {
    let db = &state.database.connection;
    let db_perm = permission::Entity::update_partial(db, patch_payload).await?;
    Ok(Json(db_perm.into()))
}

pub async fn delete_permission(
    State(state): State<AppState>,
    Json(delete_payload): Json<PermissionDeleteReq>,
) -> Result<Json<String>> {
    let db = &state.database.connection;
    let ok = permission::Entity::delete_checked(db, delete_payload.id).await?;
    if ok {
        Ok(Json("删除成功".to_string()))
    } else {
        Err(AppError::NotFound(
            "删除失败,数据库中不存在或者已删除！".to_string(),
        ))
    }
}
