use crate::result::Result;
use crate::service::identity::legacy_governance::{MutationScope, mutate};
use crate::{AppState, IdType, parse_user_id};
use anyhow::anyhow;

use crate::entity::user::{UserAddWithRoleReq, UserPatchReq, UserRolePatchReq, UserVO};
use crate::service::user::UserService;
use auth::claims::AccessTokenClaims;
use axum::extract::{Path, Query};
use axum::routing::{delete, patch};
use axum::{
    Router,
    extract::{Json, State},
    routing::get,
};

// Build routes
pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(get_page_user))
        .route(
            "/user",
            get(get_user_by_id)
                .post(add_user_with_role)
                .patch(patch_user_basic_data),
        )
        .route("/user/role", patch(patch_user_role))
        .route("/user/{user_id}", delete(delete_user_by_id))
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
pub async fn get_user_by_id(
    claim: AccessTokenClaims,
    State(state): State<AppState>,
) -> Result<Json<user::Model>> {
    let db = &state.database.connection;
    let user_id = parse_user_id(&claim.sub)?;
    if let Some((user, _role_db_opt)) = user::Entity::find_with_role_by_id(db, user_id).await? {
        Ok(Json(user))
    } else {
        Err(AppError::InternalServerError(anyhow!("获取用户失败")))
    }
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/users",
    params(PageSelectQuery),
    description ="分页获取所有用户信息",
    responses(
        (status = 200, description = "成功", body = PageSelectQueryResult<UserVO>),
    ),
))]
pub async fn get_page_user(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<UserVO>>> {
    crate::service::identity::legacy_governance::require_directory_administrator(&state, &context)
        .await?;
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;
    let (total_pages, users_db) =
        user::Entity::paginate_with_role(db, page.page_size, page.page_number).await?;

    let result = users_db
        .into_iter()
        .map(|(user_db, role_db_opt)| {
            let role = role_db_opt.as_ref();
            let mut user_vo = UserVO::from(user_db.clone());
            user_vo.role = role.cloned().map(RoleVO::from);
            user_vo
        })
        .filter(|item| item.id != Superadmin.default_id())
        .collect();

    Ok(Json(PageSelectQueryResult::<UserVO> {
        page_size,
        page_number,
        total_pages,
        data_list: result,
    }))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/user",
    request_body(
        content = UserAddWithRoleReq,
        description = "添加用户",
        content_type = "application/json"
    ),
    description ="超级管理员创建带role的用户",
    responses(
        (status = 200, description = "成功", body = UserVO),
    ),
))]
pub async fn add_user_with_role(
    State(state): State<AppState>,
    Json(payload): Json<UserAddWithRoleReq>,
) -> Result<Json<UserVO>> {
    let _ = (state, payload);
    Err(AppError::BadRequest(
        "请使用管理员邀请流程创建企业账户".into(),
    ))
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<UserRolePatchReq>,
) -> Result<Json<UserVO>> {
    mutate(
        &state,
        &context,
        "PATCH",
        "/user/role",
        MutationScope::User(payload.user_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                // 校验 role_id != 超级管理员的id
                if payload.new_role_id == Superadmin.default_id() {
                    return Err(AppError::Forbidden("禁止设置为SuperAdmin".to_string()));
                }
                let updated_user =
                    user::Entity::update_role(db, payload.user_id, payload.new_role_id).await?;
                let db_user = UserService::get_user_by_id(db, updated_user.id).await?;

                Ok(Json(db_user))
            })
        },
    )
    .await
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<UserPatchReq>,
) -> Result<Json<UserVO>> {
    mutate(
        &state,
        &context,
        "PATCH",
        "/user",
        MutationScope::User(payload.user_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                if payload.password.is_some() {
                    return Err(AppError::BadRequest(
                        "请使用账户密码修改或管理员密码重置流程".into(),
                    ));
                }
                let updated_user = user::Entity::update_basic(db, payload).await?;

                Ok(Json(updated_user.into()))
            })
        },
    )
    .await
}

#[cfg_attr(feature = "openapi", utoipa::path(
    delete,
    path = "/user/{user_id}",
    description = "删除某个用户",
    responses(
        (status = 200, description = "删除成功", body = String, example = json!("删除成功")),
        (status = 404, description = "删除失败", body = String, example = json!("用户不存在或删除失败")),
    ),
))]
pub async fn delete_user_by_id(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Path(user_id): Path<IdType>,
) -> Result<String> {
    mutate(
        &state,
        &context,
        "DELETE",
        "/user/{user_id}",
        MutationScope::User(user_id),
        move |db| {
            Box::pin(async move {
                use sea_orm::{ActiveModelTrait, EntityTrait, Set};
                let current = user::Entity::find_by_id(user_id)
                    .one(db)
                    .await?
                    .ok_or_else(|| AppError::NotFound("账户不存在".into()))?;
                let mut change: user::ActiveModel = current.into();
                change.enabled = Set(false);
                change.update(db).await?;
                Ok("账户已停用".into())
            })
        },
    )
    .await
}

use crate::entity::role::RoleEnum::Superadmin;
use crate::entity::role::RoleVO;
use crate::entity::{PageSelectQuery, PageSelectQueryResult, user};
use crate::error::AppError;
#[cfg(feature = "openapi")]
use utoipa::OpenApi;

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(
    paths(
        add_user_with_role,
        get_user_by_id,
        get_page_user,
        delete_user_by_id,
        patch_user_role,
        patch_user_basic_data
    ),
    tags ((name = "User 用户", description = "User相关CRUD"),)
)]
pub struct UserApi;
