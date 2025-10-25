use crate::AppState;
use crate::entity::role::RoleEnum::Superadmin;
use crate::entity::role::{RoleAddReq, RoleDeleteReq, RolePatchReq, RoleVO};
use crate::entity::{PageSelectQuery, PageSelectQueryResult, role};
use crate::error::AppError;
use crate::result::Result;
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set, SqlErr};
#[cfg(feature = "openapi")]
use utoipa::OpenApi;
use uuid::Uuid;

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/role",
            post(add_role)
                .get(get_page_role)
                .delete(delete_role)
                .patch(patch_role),
        )
        .route("/role/modifiable", get(get_modifiable_page_role))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/role",
    description = "添加新角色",
    request_body(
        content = RoleAddReq,
        description = "添加角色信息",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "添加成功并返回添加的角色", body = RoleVO),
        (status = 409, description = "角色名称 'xxx' 或 类型 'xxx' 已存在！")
    ),
))]
pub async fn add_role(
    State(state): State<AppState>,
    Json(insert_payload): Json<RoleAddReq>,
) -> Result<Json<RoleVO>> {
    // check role
    let db = &state.database.connection;
    let now = Utc::now().naive_utc();
    let new_role = role::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        name: Set(insert_payload.name.to_string().to_owned()),
        role_enum: Set(insert_payload.role_enum.to_owned()),
        create_time: Set(now.to_owned()),
        update_time: Set(now.to_owned()),
    };
    match new_role.insert(db).await {
        Ok(model) => Ok(Json(model.into())),
        Err(e) => {
            if let Some(sql_err) = e.sql_err() {
                match sql_err {
                    SqlErr::UniqueConstraintViolation(_) => {
                        return Err(AppError::AlreadyExists(format!(
                            "角色名称 '{}' 或 类型 '{}' 已存在！",
                            insert_payload.name, insert_payload.role_enum
                        )));
                    }
                    SqlErr::ForeignKeyConstraintViolation(_) => {
                        return Err(AppError::BadRequest("外键约束失败".to_string()));
                    }
                    // 可根据需要添加更多 SqlErr 分支
                    _ => {}
                }
            }

            // 其他未知错误，统一返回或转换为你的通用错误类型
            Err(AppError::from(e))
        }
    }
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/role",
    description = "分页获取角色",
    responses(
        (status = 200, description = "获取所有角色", body = PageSelectQueryResult<RoleVO>),
    ),
))]
pub async fn get_page_role(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<RoleVO>>> {
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;
    let paginator = role::Entity::find().paginate(db, page.page_size);

    let total_pages = paginator.num_pages().await?;
    let roles_db = paginator.fetch_page(page.page_number - 1).await?;
    Ok(Json(PageSelectQueryResult::<RoleVO> {
        page_size,
        page_number,
        total_pages,
        data_list: roles_db.into_iter().map(RoleVO::from).collect(),
    }))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/role/modifiable",
    description = "分页获取可修改的角色",
    responses(
        (status = 200, description = "", body = PageSelectQueryResult<RoleVO>),
    ),
))]
pub async fn get_modifiable_page_role(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<RoleVO>>> {
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;
    let paginator = role::Entity::find().paginate(db, page.page_size);

    let total_pages = paginator.num_pages().await?;
    let roles_db = paginator.fetch_page(page.page_number - 1).await?;

    let super_admin_id = Superadmin.default_id();
    Ok(Json(PageSelectQueryResult::<RoleVO> {
        page_size,
        page_number,
        total_pages,
        data_list: roles_db
            .into_iter()
            .filter(|role| role.id != super_admin_id)
            .map(RoleVO::from)
            .collect(),
    }))
}

/// 更新角色信息
#[cfg_attr(feature = "openapi", utoipa::path(
    patch,
    path = "/role",
    description = "更新角色",
    request_body(
        content = RolePatchReq,
        description = "更新角色信息",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "更新角色信息", body = RoleVO),
        (status = 404, description = "找不到该角色"),
    ),
))]
pub async fn patch_role(
    State(state): State<AppState>,
    Json(patch_payload): Json<RolePatchReq>,
) -> Result<Json<RoleVO>> {
    let db = &state.database.connection;
    // // 校验id 和 更新冲突
    let role_db = role::Entity::find_by_id(patch_payload.id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("找不到该角色".to_string()))?;
    // 更新
    let mut new_role: role::ActiveModel = role_db.into();
    new_role.name = Set(patch_payload.name.to_owned());
    new_role.role_enum = Set(patch_payload.role_enum.to_owned());
    new_role.update_time = Set(Utc::now().naive_utc());

    let db_role = new_role.update(db).await?;

    Ok(Json(db_role.into()))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    delete,
    path = "/role",
    description = "删除角色",
    request_body(
        content = RoleDeleteReq,
        description = "删除角色",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "删除成功", body = String, example = json!("删除成功")),
        (status = 404, description = "删除失败", body = String, example = json!("删除失败,数据库中不存在或者已删除！")),
    ),
))]
pub async fn delete_role(
    State(state): State<AppState>,
    Json(delete_payload): Json<RoleDeleteReq>,
) -> Result<Json<String>> {
    let db = &state.database.connection;
    let res = role::Entity::delete_by_id(delete_payload.id)
        .exec(db)
        .await?;
    if res.rows_affected == 1 {
        Ok(Json("删除成功".to_string()))
    } else {
        Err(AppError::NotFound(
            "删除失败,数据库中不存在或者已删除！".to_string(),
        ))
    }
}

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(paths(delete_role, patch_role, add_role, get_page_role))]
pub struct RoleApi;
