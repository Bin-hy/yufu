use crate::AppState;
use crate::entity::authority::{
    AuthorityDeleteReq, AuthorityInsertReq, AuthorityPatchReq, AuthorityVO,
};
use crate::entity::{PageSelectQuery, PageSelectQueryResult, authority};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use axum::extract::{Query, State};
use axum::routing::post;
use axum::{Json, Router};
#[cfg(feature = "openapi")]
use utoipa::OpenApi;

pub fn get_auth_routes() -> Router<AppState> {
    Router::new().route(
        "/authority",
        post(add_authority)
            .get(get_page_authorities)
            .patch(patch_authority)
            .delete(delete_authority),
    )
    // 设置权限
}

// 增加权限
#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/authority",
    request_body(
        content = AuthorityInsertReq,
        description = "新的权限信息",
        content_type = "application/json"
    ),
    description = "添加权限",
    responses(
        (status = 200, description = "用户注册成功并返回 token", body = AuthorityVO),
        (status = 409, description = "权限名称 'xxx' 或 权限路径 'xxxx' 已存在！")
    ),
))]
pub async fn add_authority(
    State(state): State<AppState>,
    Json(insert_payload): Json<AuthorityInsertReq>,
) -> Result<Json<AuthorityVO>> {
    let db = &state.database.connection;
    let new_authority = authority::Entity::insert_one(db, insert_payload)
        .await
        .into_app_with_conflict_msg("权限名称或路径已存在")?;
    //
    crate::service::authority::Service::ensure_sadmin_authorities(db).await?;
    Ok(Json(new_authority.into()))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/authority",
    description = "分页获取权限清单",
    responses(
        (status = 200, description = "分页获取权限路径信息", body = PageSelectQueryResult<AuthorityVO>),
    ),
))]
pub async fn get_page_authorities(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<AuthorityVO>>> {
    let db = &state.database.connection;
    let (total_pages, authorities_db) =
        authority::Entity::paginate(db, page.page_size, page.page_number).await?;
    Ok(Json(PageSelectQueryResult::<AuthorityVO> {
        page_size: page.page_size,
        page_number: page.page_number,
        total_pages,
        data_list: authorities_db.into_iter().map(AuthorityVO::from).collect(),
    }))
}
#[cfg_attr(feature = "openapi", utoipa::path(
    patch,
    path = "/authority",
    description = "更新权限",
    request_body(
        content = AuthorityPatchReq,
        description = "更新权限信息",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "更新权限细节", body = AuthorityVO),
        (status = 404, description = "找不到该权限"),
    ),
))]
pub async fn patch_authority(
    State(state): State<AppState>,
    Json(patch_payload): Json<AuthorityPatchReq>,
) -> Result<Json<AuthorityVO>> {
    let db = &state.database.connection;
    // // 校验id 和 更新冲突
    let db_authority = authority::Entity::update_partial(db, patch_payload).await?;

    Ok(Json(db_authority.into()))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    delete,
    path = "/authority",
    description = "删除某项权限",
    request_body(
        content = AuthorityDeleteReq,
        description = "删除权限信息",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "删除成功", body = String, example = json!("删除成功")),
        (status = 404, description = "删除失败", body = String, example = json!("删除失败,数据库中不存在或者已删除！")),
    ),
))]
pub async fn delete_authority(
    State(state): State<AppState>,
    Json(delete_payload): Json<AuthorityDeleteReq>,
) -> Result<Json<String>> {
    let db = &state.database.connection;
    let ok = authority::Entity::delete_checked(db, delete_payload.id).await?;
    if ok {
        Ok(Json("删除成功".to_string()))
    } else {
        Err(AppError::NotFound(
            "删除失败,数据库中不存在或者已删除！".to_string(),
        ))
    }
}

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(paths(delete_authority, patch_authority, add_authority, get_page_authorities))]
pub struct AuthorityApi;
