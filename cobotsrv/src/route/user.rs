use anyhow::anyhow;
use crate::AppState;
use crate::result::Result;

use crate::entity::user::{UserPatchReq, UserRolePatchReq, UserVO};
use crate::service::user::UserService;
use auth::claims::Claims;
use axum::extract::Query;
use axum::routing::patch;
use axum::{
    Router,
    extract::{Json, State},
    routing::get,
};
use sea_orm::{ActiveModelTrait, EntityTrait, NotSet, PaginatorTrait, Set};

// Build routes
pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(get_page_user))
        .route("/user", get(get_user_by_id))
        .route("/user", patch(patch_user_basic_data))
        .route("/user/role", patch(patch_user_role))
}

pub fn get_public_routes() -> Router<AppState> {
    Router::new()
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/user",
    description = "获取当前用户信息",
    responses(
        (status = 200, description = "成功", body = UserVO),
    ),
))]
pub async fn get_user_by_id(claim: Claims, State(state): State<AppState>) -> Result<Json<UserVO>> {
    let db = &state.database.connection;
    let user_vo = UserService::get_user_by_id(db, claim.id.to_string()).await?;
    Ok(Json(user_vo))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/users",
    params(PageSelectQuery),
    description ="分页获取所有用户信息",
    responses(
        (status = 200, description = "成功", body = UserVO),
    ),
))]
pub async fn get_page_user(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<UserVO>>> {
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;
    let paginator = user::Entity::find().paginate(db, page.page_size);
    let total_pages = paginator.num_pages().await?;

    let paginator = user::Entity::find()
        .find_also_related(role::Entity)
        .paginate(db, page.page_size);
    let users_db = paginator.fetch_page(page.page_number - 1).await?;

    let result = users_db
        .iter()
        .map(|(user_db, role_db_opt)| {
            let role = role_db_opt.as_ref().unwrap();
            let mut user_vo = UserVO::from(user_db.clone());
            user_vo.role = Some(RoleVO::from(role.clone()));
            user_vo
        })
        .collect();

    Ok(Json(PageSelectQueryResult::<UserVO> {
        page_size,
        page_number,
        total_pages,
        data_list: result,
    }))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    patch,
    path = "/user/role",
    request_body(
        content = UserRolePatchReq,
        description = "更新权限信息",
        content_type = "application/json"
    ),
    description ="更新用户信息角色,无法设为超级管理员",
    responses(
        (status = 200, description = "成功", body = UserVO),
    ),
))]
pub async fn patch_user_role(
    State(state): State<AppState>,
    Json(payload): Json<UserRolePatchReq>,
) -> Result<Json<UserVO>> {
    let db = &state.database.connection;
    // 校验 role_id != 超级管理员的id
    if payload.new_role_id.clone() == RoleEnum::Superadmin.default_id() {
        return Err(AppError::Forbidden("禁止设置为SuperAdmin".to_string()));
    }
    let user_db = user::Entity::find_by_id(payload.user_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let role_db = role::Entity::find_by_id(payload.new_role_id.clone())
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Role not found".to_string()))?;
    let mut user_db_am: user::ActiveModel = user_db.into();
    user_db_am.role_id = Set(role_db.id.to_owned());

    let updated_user = user_db_am.update(db).await?;
    let db_user = UserService::get_user_by_id(db, updated_user.id).await?;

    Ok(Json(db_user))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    patch,
    path = "/user",
    request_body(
        content = UserRolePatchReq,
        description = "用户更新数据",
        content_type = "application/json"
    ),
    description ="更新用户基础信息",
    responses(
        (status = 200, description = "成功", body = UserVO),
    ),
))]
pub async fn patch_user_basic_data(
    State(state): State<AppState>,
    Json(payload): Json<UserPatchReq>,
) -> Result<Json<UserVO>> {
    let db = &state.database.connection;
    let user_dbm = user::Entity::find_by_id(payload.user_id.clone())
        .one(db)
        .await
        .unwrap()
        .ok_or(AppError::NotFound("找不到用户".to_string()))?;
    let mut user_dbam: user::ActiveModel = user_dbm.into();
    user_dbam.nickname = payload
        .nickname
        .clone()
        .map(|s| Set(Some(s)))
        .unwrap_or(NotSet);
    user_dbam.password = payload
        .password
        .clone()
        .map(|s| Set(Some(s)))
        .unwrap_or(NotSet);
    let updated_user = user_dbam.update(db).await?;

    Ok(Json(updated_user.into()))
}

use crate::entity::role::{RoleEnum, RoleVO};
use crate::entity::{PageSelectQuery, PageSelectQueryResult, role, user};
use crate::error::AppError;
#[cfg(feature = "openapi")]
use utoipa::OpenApi;

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(
    paths(
        get_user_by_id,
        get_page_user,
        patch_user_role,
        patch_user_basic_data
    ),
    tags ((name = "User 用户", description = "User相关CRUD"),)
)]
pub struct UserApi;
