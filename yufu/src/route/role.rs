use crate::entity::authority::AuthorityVO;
use crate::entity::authority_role::{
    AuthRoleAddReq, AuthRoleBatchAddReq, AuthRoleBatchDeleteReq, AuthRoleDeleteReq, AuthorityRoleVO,
};
use crate::entity::role::RoleEnum::Superadmin;
use crate::entity::role::{RoleAddReq, RoleDeleteReq, RolePatchReq, RoleVO};
use crate::entity::{PageSelectQuery, PageSelectQueryResult, authority_role, role};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use crate::service::identity::legacy_governance::{MutationScope, mutate};
use crate::{AppState, IdType};
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use sea_orm::{DeleteResult, SqlErr};
#[cfg(feature = "openapi")]
use utoipa::OpenApi;

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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(insert_payload): Json<RoleAddReq>,
) -> Result<Json<RoleVO>> {
    mutate(
        &state,
        &context,
        "POST",
        "/role",
        MutationScope::Registry,
        move |db| {
            let insert_payload = insert_payload.clone();
            Box::pin(async move {
                // check role

                let role_db = role::Entity::insert_one(db, insert_payload)
                    .await
                    .into_app_with_conflict_msg("角色名称已存在！")?;
                Ok(Json(role_db.into()))
            })
        },
    )
    .await
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<RoleVO>>> {
    crate::service::identity::legacy_governance::require_directory_administrator(&state, &context)
        .await?;
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;
    let (total_pages, roles_db) =
        role::Entity::paginate(db, page.page_size, page.page_number).await?;
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<RoleVO>>> {
    crate::service::identity::legacy_governance::require_directory_administrator(&state, &context)
        .await?;
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;
    let (total_pages, roles_db) =
        role::Entity::paginate(db, page.page_size, page.page_number).await?;

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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(patch_payload): Json<RolePatchReq>,
) -> Result<Json<RoleVO>> {
    mutate(
        &state,
        &context,
        "PATCH",
        "/role",
        MutationScope::Role(patch_payload.id),
        move |db| {
            let patch_payload = patch_payload.clone();
            Box::pin(async move {
                // // 校验id 和 更新冲突
                let db_role = role::Entity::update_name(db, patch_payload).await?;

                Ok(Json(db_role.into()))
            })
        },
    )
    .await
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(delete_payload): Json<RoleDeleteReq>,
) -> Result<Json<String>> {
    mutate(
        &state,
        &context,
        "DELETE",
        "/role",
        MutationScope::Role(delete_payload.id),
        move |db| {
            let delete_payload = delete_payload.clone();
            Box::pin(async move {
                let ok = role::Entity::delete_checked(db, delete_payload.id).await?;
                if ok {
                    Ok(Json("删除成功".to_string()))
                } else {
                    Err(AppError::NotFound(
                        "删除失败,数据库中不存在或者已删除！".to_string(),
                    ))
                }
            })
        },
    )
    .await
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<AuthRoleAddReq>,
) -> Result<Json<AuthorityRoleVO>> {
    mutate(
        &state,
        &context,
        "POST",
        "/role_authority",
        MutationScope::Role(payload.role_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                // 检查 role_id, authority_id 合法性
                let authority_vo = crate::service::authority::Service::get_authority_vo(
                    db,
                    payload.clone().authority_id,
                )
                .await?;
                let role_vo =
                    crate::service::role::Service::get_role_vo(db, payload.clone().role_id).await?;

                match authority_role::Entity::add_one(db, payload.role_id, payload.authority_id)
                    .await
                {
                    Ok(model) => {
                        let mut auth_role_vo = AuthorityRoleVO::from(model);
                        auth_role_vo = auth_role_vo.set_role_authority(role_vo, authority_vo);
                        Ok(Json(auth_role_vo))
                    }
                    Err(e) => {
                        if let Some(sql_err) = e.sql_err() {
                            match sql_err {
                                SqlErr::UniqueConstraintViolation(_) => {
                                    return Err(AppError::AlreadyExists(
                                        "权限已存在！".to_string(),
                                    ));
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
            })
        },
    )
    .await
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<AuthRoleBatchAddReq>,
) -> Result<Json<Vec<AuthorityRoleVO>>> {
    mutate(
        &state,
        &context,
        "POST",
        "/role_authority/batch",
        MutationScope::Role(payload.role_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                // 校验 role 是否存在
                let role_vo =
                    crate::service::role::Service::get_role_vo(db, payload.role_id).await?;

                // 校验每个 authority 是否存在，并收集对象
                let mut authorities = Vec::new();
                for aid in &payload.authority_ids {
                    let auth_vo =
                        crate::service::authority::Service::get_authority_vo(db, *aid).await?;
                    authorities.push(auth_vo);
                }

                // 开启事务，确保全部插入成功或全部回滚
                let inserted = authority_role::Entity::add_batch_tx(
                    db,
                    payload.role_id,
                    authorities.iter().map(|a| a.id).collect(),
                )
                .await;
                match inserted {
                    Ok(models) => {
                        let mut inserted_vos = Vec::new();
                        for (i, model) in models.into_iter().enumerate() {
                            let mut vo = AuthorityRoleVO::from(model);
                            vo = vo.set_role_authority(role_vo.clone(), authorities[i].clone());
                            inserted_vos.push(vo);
                        }
                        Ok(Json(inserted_vos))
                    }
                    Err(e) => {
                        if let Some(sql_err) = e.sql_err() {
                            match sql_err {
                                SqlErr::UniqueConstraintViolation(_) => {
                                    return Err(AppError::AlreadyExists(
                                        "权限已存在！".to_string(),
                                    ));
                                }
                                SqlErr::ForeignKeyConstraintViolation(_) => {
                                    return Err(AppError::BadRequest("外键约束失败".to_string()));
                                }
                                _ => {}
                            }
                        }
                        Err(AppError::from(e))
                    }
                }
            })
        },
    )
    .await
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<AuthRoleDeleteReq>,
) -> Result<Json<String>> {
    mutate(
        &state,
        &context,
        "DELETE",
        "/role_authority",
        MutationScope::Role(payload.role_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                // 删除 role_id + authority_id 对应的记录
                let delete_result: DeleteResult =
                    authority_role::Entity::delete_pair(db, payload.role_id, payload.authority_id)
                        .await
                        .map_err(AppError::from)?;

                if delete_result.rows_affected == 0 {
                    return Err(AppError::NotFound(format!(
                        "未找到角色 {} 对应的权限 {}",
                        payload.role_id, payload.authority_id
                    )));
                }

                Ok(Json("删除成功".to_string()))
            })
        },
    )
    .await
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<AuthRoleBatchDeleteReq>,
) -> Result<Json<String>> {
    mutate(
        &state,
        &context,
        "DELETE",
        "/role_authority/batch",
        MutationScope::Role(payload.role_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                // 执行批量删除
                let delete_result: DeleteResult = authority_role::Entity::delete_batch(
                    db,
                    payload.role_id,
                    payload.authority_ids.clone(),
                )
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
            })
        },
    )
    .await
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Path(role_id): Path<IdType>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<AuthorityVO>>> {
    crate::service::identity::legacy_governance::require_directory_administrator(&state, &context)
        .await?;
    let db = &state.database.connection;
    let page_size = page.page_size;
    let page_number = page.page_number;

    // 先查询 role，然后通过关联查询 authorities
    let (total_pages, page_records) =
        authority_role::Entity::paginate_authorities_by_role(db, role_id, page_size, page_number)
            .await?;

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
