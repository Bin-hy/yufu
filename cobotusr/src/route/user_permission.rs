use crate::AppState;
use crate::entity::permission;
use crate::entity::permission::PermissionVO;
use crate::entity::user_permission::{
    self, UserPermAddReq, UserPermDeleteReq, UserPermPatchReq, UserPermVO,
};
use crate::entity::{PageSelectQuery, PageSelectQueryResult};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::routing::{delete, get, patch, post};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/user_permission_override", post(add_user_permission))
        .route("/user_permission_override", delete(delete_user_permission))
        .route("/user_permission_override", patch(patch_user_permission))
        .route("/user_permission_override", get(get_user_permissions_page))
}

/// CREATE: POST /user_permission_override
pub async fn add_user_permission(
    State(state): State<AppState>,
    Json(payload): Json<UserPermAddReq>,
) -> Result<Json<UserPermVO>> {
    let db = &state.database.connection;

    let user_vo = crate::service::user::UserService::get_user_by_id(db, payload.user_id).await?;
    let perm_vo = get_permission_vo(db, payload.permission_id).await?;

    let inserted = user_permission::Entity::upsert_override(
        db,
        payload.user_id,
        payload.permission_id,
        payload.allow,
    )
    .await
    .into_app_with_conflict_msg("覆写条目已存在！")?;

    Ok(Json(UserPermVO {
        user_id: inserted.user_id,
        permission_id: inserted.permission_id,
        allow: inserted.allow,
        create_time: inserted.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        update_time: inserted.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        user_vo: Some(user_vo),
        permission_vo: Some(perm_vo),
    }))
}

/// DELETE: DELETE /user_permission_override
pub async fn delete_user_permission(
    State(state): State<AppState>,
    Json(payload): Json<UserPermDeleteReq>,
) -> Result<Json<String>> {
    let db = &state.database.connection;
    let res = user_permission::Entity::delete_pair(db, payload.user_id, payload.permission_id)
        .await
        .map_err(AppError::from)?;
    if res.rows_affected == 0 {
        Err(AppError::NotFound(
            "删除失败,数据库中不存在或者已删除！".to_string(),
        ))
    } else {
        Ok(Json("删除成功".to_string()))
    }
}

/// PATCH: PATCH /user_permission_override
pub async fn patch_user_permission(
    State(state): State<AppState>,
    Json(payload): Json<UserPermPatchReq>,
) -> Result<Json<UserPermVO>> {
    let db = &state.database.connection;

    let existing = user_permission::Entity::find_pair(db, payload.user_id, payload.permission_id)
        .await?
        .ok_or_else(|| AppError::NotFound("找不到该 用户 | 权限".to_string()))?;

    let mut am: user_permission::ActiveModel = existing.into();
    am.allow = Set(payload.allow);
    am.update_time = Set(Utc::now().naive_utc());
    let updated = am.update(db).await?;

    let user_vo = crate::service::user::UserService::get_user_by_id(db, updated.user_id).await?;
    let perm_vo = get_permission_vo(db, updated.permission_id).await?;

    Ok(Json(UserPermVO {
        user_id: updated.user_id,
        permission_id: updated.permission_id,
        allow: updated.allow,
        create_time: updated.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        update_time: updated.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        user_vo: Some(user_vo),
        permission_vo: Some(perm_vo),
    }))
}

/// LIST: GET /user_permission_override
pub async fn get_user_permissions_page(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<UserPermVO>>> {
    let db = &state.database.connection;
    let (total_pages, records) =
        user_permission::Entity::paginate(db, page.page_size, page.page_number).await?;
    let mut data_list = Vec::with_capacity(records.len());
    for up in records {
        let user_vo = crate::service::user::UserService::get_user_by_id(db, up.user_id).await?;
        let perm_vo = get_permission_vo(db, up.permission_id).await?;
        data_list.push(UserPermVO {
            user_id: up.user_id,
            permission_id: up.permission_id,
            allow: up.allow,
            create_time: up.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            update_time: up.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            user_vo: Some(user_vo),
            permission_vo: Some(perm_vo),
        });
    }
    Ok(Json(PageSelectQueryResult {
        page_size: page.page_size,
        page_number: page.page_number,
        total_pages,
        data_list,
    }))
}

async fn get_permission_vo(
    db: &sea_orm::DatabaseConnection,
    perm_id: crate::IdType,
) -> Result<PermissionVO> {
    match permission::Entity::find_by_id(perm_id).one(db).await? {
        Some(p) => Ok(PermissionVO::from(p)),
        None => Err(AppError::NotFound("权限规则不存在".to_string())),
    }
}
