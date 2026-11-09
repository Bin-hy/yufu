use crate::entity::permission::{self, PermissionVO};
use crate::entity::role::Entity as RoleEntity;
use crate::entity::role::RoleVO;
use crate::entity::role_permission::{
    self, RolePermAddReq, RolePermBatchAddReq, RolePermBatchDeleteReq, RolePermDeleteReq,
    RolePermissionVO,
};
use crate::entity::{PageSelectQuery, PageSelectQueryResult};
use crate::error::AppError;
use crate::result::Result;
use crate::service::identity::legacy_governance::{MutationScope, mutate};
use crate::{AppState, IdType};
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use sea_orm::EntityTrait;

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/role_permission",
            post(add_role_permission).delete(delete_role_permission),
        )
        .route(
            "/role_permission/batch",
            post(add_role_permissions).delete(delete_role_permissions),
        )
        .route(
            "/role_permission/{role_id}",
            get(get_role_permissions_by_role_id),
        )
}

/// Single add: POST /role_permission
pub async fn add_role_permission(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<RolePermAddReq>,
) -> Result<Json<RolePermissionVO>> {
    mutate(
        &state,
        &context,
        "POST",
        "/role_permission",
        MutationScope::Role(payload.role_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                let role_vo = get_role_vo(db, payload.role_id).await?;
                let perm_vo = get_permission_vo(db, payload.permission_id).await?;

                let inserted =
                    role_permission::Entity::add_one(db, payload.role_id, payload.permission_id)
                        .await
                        .map_err(|e| {
                            use sea_orm::SqlErr;
                            if let Some(SqlErr::UniqueConstraintViolation(_)) = e.sql_err() {
                                AppError::AlreadyExists("角色权限关联已存在".to_string())
                            } else {
                                AppError::from(e)
                            }
                        })?;

                Ok(Json(
                    RolePermissionVO::from(inserted).set_role_permission(role_vo, perm_vo),
                ))
            })
        },
    )
    .await
}

/// Batch add: POST /role_permission/batch
pub async fn add_role_permissions(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<RolePermBatchAddReq>,
) -> Result<Json<Vec<RolePermissionVO>>> {
    mutate(
        &state,
        &context,
        "POST",
        "/role_permission/batch",
        MutationScope::Role(payload.role_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                let role_vo = get_role_vo(db, payload.role_id).await?;
                let inserted = role_permission::Entity::add_batch_tx(
                    db,
                    payload.role_id,
                    payload.permission_ids.clone(),
                )
                .await
                .map_err(|e| {
                    use sea_orm::SqlErr;
                    if let Some(SqlErr::UniqueConstraintViolation(_)) = e.sql_err() {
                        AppError::AlreadyExists("部分角色权限关联已存在".to_string())
                    } else {
                        AppError::from(e)
                    }
                })?;

                let mut vos = Vec::with_capacity(inserted.len());
                for (i, m) in inserted.into_iter().enumerate() {
                    let perm_id = payload
                        .permission_ids
                        .get(i)
                        .copied()
                        .unwrap_or(m.permission_id);
                    let perm_vo = get_permission_vo(db, perm_id).await?;
                    vos.push(
                        RolePermissionVO::from(m).set_role_permission(role_vo.clone(), perm_vo),
                    );
                }
                Ok(Json(vos))
            })
        },
    )
    .await
}

/// Single delete: DELETE /role_permission
pub async fn delete_role_permission(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<RolePermDeleteReq>,
) -> Result<Json<String>> {
    mutate(
        &state,
        &context,
        "DELETE",
        "/role_permission",
        MutationScope::Role(payload.role_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                let res = role_permission::Entity::delete_pair(
                    db,
                    payload.role_id,
                    payload.permission_id,
                )
                .await
                .map_err(AppError::from)?;
                if res.rows_affected == 0 {
                    Err(AppError::NotFound(
                        "删除失败,数据库中不存在或者已删除！".to_string(),
                    ))
                } else {
                    Ok(Json("删除成功".to_string()))
                }
            })
        },
    )
    .await
}

/// Batch delete: DELETE /role_permission/batch
pub async fn delete_role_permissions(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(payload): Json<RolePermBatchDeleteReq>,
) -> Result<Json<String>> {
    mutate(
        &state,
        &context,
        "DELETE",
        "/role_permission/batch",
        MutationScope::Role(payload.role_id),
        move |db| {
            let payload = payload.clone();
            Box::pin(async move {
                let res = role_permission::Entity::delete_batch(
                    db,
                    payload.role_id,
                    payload.permission_ids,
                )
                .await
                .map_err(AppError::from)?;
                if res.rows_affected == 0 {
                    Err(AppError::NotFound(
                        "删除失败,数据库中不存在或者已删除！".to_string(),
                    ))
                } else {
                    Ok(Json("删除成功".to_string()))
                }
            })
        },
    )
    .await
}

/// List permissions by role: GET /role_permission/{role_id}
pub async fn get_role_permissions_by_role_id(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Path(role_id): Path<IdType>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<PermissionVO>>> {
    crate::service::identity::legacy_governance::require_directory_administrator(&state, &context)
        .await?;
    let db = &state.database.connection;
    let (total_pages, records) = role_permission::Entity::paginate_permissions_by_role(
        db,
        role_id,
        page.page_size,
        page.page_number,
    )
    .await?;
    let data_list: Vec<PermissionVO> = records
        .into_iter()
        .filter_map(|(_, perm)| perm.map(PermissionVO::from))
        .collect();
    Ok(Json(PageSelectQueryResult {
        page_size: page.page_size,
        page_number: page.page_number,
        total_pages,
        data_list,
    }))
}

// -- helpers --

async fn get_role_vo(db: &impl sea_orm::ConnectionTrait, role_id: IdType) -> Result<RoleVO> {
    match RoleEntity::find_by_id(role_id).one(db).await? {
        Some(r) => Ok(RoleVO::from(r)),
        None => Err(AppError::NotFound("角色不存在".to_string())),
    }
}

async fn get_permission_vo(
    db: &impl sea_orm::ConnectionTrait,
    perm_id: IdType,
) -> Result<PermissionVO> {
    match permission::Entity::find_by_id(perm_id).one(db).await? {
        Some(p) => Ok(PermissionVO::from(p)),
        None => Err(AppError::NotFound("权限规则不存在".to_string())),
    }
}
