use crate::AppState;
use crate::entity::authority::AuthorityVO;
use crate::entity::authority_role::{
    AuthRoleAddReq, AuthRoleBatchAddReq, AuthRoleBatchDeleteReq, AuthRoleDeleteReq, AuthorityRoleVO,
};
use crate::entity::role::RoleEnum::Superadmin;
use crate::entity::role::{RoleAddReq, RoleDeleteReq, RolePatchReq, RoleVO};
use crate::entity::{PageSelectQuery, PageSelectQueryResult, authority, authority_role, role};
use crate::error::AppError;
use crate::result::Result;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DeleteResult, EntityTrait, PaginatorTrait, QueryFilter, Set,
    SqlErr, TransactionTrait,
};
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
        .route(
            "/role_authority",
            post(add_role_authority).delete(delete_role_authority),
        )
        .route("/role/modifiable", get(get_modifiable_page_role))
        .route(
            "/role_authority/{role_id}",
            get(get_role_authority_by_role_id),
        )
        .route(
            "/role_authority/batch",
            post(add_role_authorities).delete(delete_role_authorities),
        )
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
    params(PageSelectQuery),
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

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/role_authority",
    description = "给角色添加权限",
    request_body(
        content = AuthRoleAddReq,
        description = "添加角色信息",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "添加成功", body = AuthorityRoleVO),
        (status = 409, description = "权限已存在")
    ),
))]
pub async fn add_role_authority(
    State(state): State<AppState>,
    Json(payload): Json<AuthRoleAddReq>,
) -> Result<Json<AuthorityRoleVO>> {
    let db = &state.database.connection;
    // 检查 role_id, authority_id 合法性
    let authority_vo =
        crate::service::authority::Service::get_authority_vo(db, payload.clone().authority_id)
            .await?;
    let role_vo = crate::service::role::Service::get_role_vo(db, payload.clone().role_id).await?;

    let new_auth_role = authority_role::ActiveModel {
        role_id: Set(payload.role_id.to_owned()),
        authority_id: Set(payload.authority_id.to_owned()),
        create_time: Set(Utc::now().naive_utc()),
        update_time: Set(Utc::now().naive_utc()),
    };

    match new_auth_role.insert(db).await {
        Ok(model) => {
            let mut auth_role_vo = AuthorityRoleVO::from(model);
            auth_role_vo = auth_role_vo.set_role_authority(role_vo, authority_vo);
            Ok(Json(auth_role_vo))
        }
        Err(e) => {
            if let Some(sql_err) = e.sql_err() {
                match sql_err {
                    SqlErr::UniqueConstraintViolation(_) => {
                        return Err(AppError::AlreadyExists("权限已存在！".to_string()));
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
    post,
    path = "/role_authority/batch",
    description = "批量给角色添加权限",
    request_body(
        content = AuthRoleBatchAddReq,
        description = "批量添加的权限信息",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "添加成功", body = Vec<AuthorityRoleVO>),
        (status = 409, description = "权限已存在")
    ),
))]
pub async fn add_role_authorities(
    State(state): State<AppState>,
    Json(payload): Json<AuthRoleBatchAddReq>,
) -> Result<Json<Vec<AuthorityRoleVO>>> {
    let db = &state.database.connection;

    // 校验 role 是否存在
    let role_vo = crate::service::role::Service::get_role_vo(db, payload.role_id.clone()).await?;

    // 校验每个 authority 是否存在，并收集对象
    let mut authorities = Vec::new();
    for aid in &payload.authority_ids {
        let auth_vo = crate::service::authority::Service::get_authority_vo(db, aid.clone()).await?;
        authorities.push(auth_vo);
    }

    // 开启事务，确保全部插入成功或全部回滚
    let txn = db.begin().await?;

    let mut inserted_vos = Vec::new();
    for authority_vo in &authorities {
        let new_auth_role = authority_role::ActiveModel {
            role_id: Set(payload.role_id.clone()),
            authority_id: Set(authority_vo.id.clone()),
            create_time: Set(Utc::now().naive_utc()),
            update_time: Set(Utc::now().naive_utc()),
        };

        match new_auth_role.insert(&txn).await {
            Ok(model) => {
                let mut vo = AuthorityRoleVO::from(model);
                vo = vo.set_role_authority(role_vo.clone(), authority_vo.clone());
                inserted_vos.push(vo);
            }
            Err(e) => {
                if let Some(sql_err) = e.sql_err() {
                    match sql_err {
                        SqlErr::UniqueConstraintViolation(_) => {
                            return Err(AppError::AlreadyExists(format!(
                                "权限 '{}' 已存在于角色中！",
                                authority_vo.name
                            )));
                        }
                        SqlErr::ForeignKeyConstraintViolation(_) => {
                            return Err(AppError::BadRequest("外键约束失败".to_string()));
                        }
                        _ => {}
                    }
                }
                return Err(AppError::from(e));
            }
        }
    }

    txn.commit().await?;

    Ok(Json(inserted_vos))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    delete,
    path = "/role_authority",
    description = "给角色删除权限",
    request_body(
        content = AuthRoleDeleteReq,
        description = "添加角色信息",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "删除成功", body = String ,example="删除成功"),
        (status = 404, description = "删除失败", body = String ,example="未找到角色 {} 对应的权限 {}"),
    ),
))]
pub async fn delete_role_authority(
    State(state): State<AppState>,
    Json(payload): Json<AuthRoleDeleteReq>,
) -> Result<Json<String>> {
    let db = &state.database.connection;

    // 删除 role_id + authority_id 对应的记录
    let delete_result: DeleteResult = authority_role::Entity::delete_many()
        .filter(authority_role::Column::RoleId.eq(payload.role_id.clone()))
        .filter(authority_role::Column::AuthorityId.eq(payload.authority_id.clone()))
        .exec(db)
        .await
        .map_err(AppError::from)?;

    if delete_result.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "未找到角色 {} 对应的权限 {}",
            payload.role_id, payload.authority_id
        )));
    }

    Ok(Json("删除成功".to_string()))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    delete,
    path = "/role_authority/batch",
    description = "批量删除角色权限",
    request_body(
        content = AuthRoleBatchDeleteReq,
        description = "批量删除角色的权限",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "批量删除成功", body = String, example="删除成功"),
        (status = 404, description = "删除失败", body = String, example="未找到要删除的权限")
    ),
))]
pub async fn delete_role_authorities(
    State(state): State<AppState>,
    Json(payload): Json<AuthRoleBatchDeleteReq>,
) -> Result<Json<String>> {
    let db = &state.database.connection;

    // 执行批量删除
    let delete_result: DeleteResult = authority_role::Entity::delete_many()
        .filter(authority_role::Column::RoleId.eq(payload.role_id.clone()))
        .filter(authority_role::Column::AuthorityId.is_in(payload.authority_ids.clone()))
        .exec(db)
        .await
        .map_err(AppError::from)?;

    if delete_result.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "未找到角色 {} 对应的权限记录",
            payload.role_id
        )));
    }

    Ok(Json(format!(
        "成功删除 {} 条记录",
        delete_result.rows_affected
    )))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/role_authority/{role_id}",
    params(PageSelectQuery),
    description ="分页获取某个角色的所有权限信息",
    responses(
        (status = 200, description = "成功", body = PageSelectQueryResult<AuthorityVO>),
    ),
))]
pub async fn get_role_authority_by_role_id(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<AuthorityVO>>> {
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;

    // 先查询 role，然后通过关联查询 authorities
    let paginator = authority_role::Entity::find()
        .filter(authority_role::Column::RoleId.eq(role_id.clone()))
        .find_also_related(authority::Entity)
        .paginate(db, page_size);

    let total_pages = paginator.num_pages().await?;
    let page_records = paginator.fetch_page(page_number - 1).await?;

    // 提取 authority 数据
    let data_list = page_records
        .into_iter()
        .filter_map(|(_, authority)| authority)
        .map(AuthorityVO::from)
        .collect::<Vec<_>>();

    Ok(Json(PageSelectQueryResult::<AuthorityVO> {
        page_size,
        page_number,
        total_pages,
        data_list,
    }))
}

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(paths(
    delete_role,
    patch_role,
    add_role,
    get_page_role,
    get_modifiable_page_role,
    delete_role_authority,
    delete_role_authorities,
    get_role_authority_by_role_id,
    add_role_authority,
    add_role_authorities
))]
pub struct RoleApi;
