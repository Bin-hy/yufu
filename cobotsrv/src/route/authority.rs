use crate::AppState;
use crate::entity::authority::{
    AuthorityDeleteReq, AuthorityInsertReq, AuthorityPatchReq, AuthorityVO,
};
use crate::entity::{PageSelectQuery, PageSelectQueryResult, authority};
use crate::error::AppError;
use crate::result::Result;
use axum::extract::{Query, State};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set, SqlErr};
#[cfg(feature = "openapi")]
use utoipa::OpenApi;
use uuid::Uuid;

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
    let now = Utc::now().naive_utc();

    let new_authority = authority::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        name: Set(insert_payload.name.clone()),
        authority_uri: Set(insert_payload.authority_uri.clone()),
        create_time: Set(now),
        update_time: Set(now),
    };

    match new_authority.insert(db).await {
        Ok(model) => Ok(Json(model.into())),
        Err(e) => {
            // 优先使用 sea_orm 提供的 sql_err() 来判断常见 SQL 错误
            if let Some(sql_err) = e.sql_err() {
                match sql_err {
                    SqlErr::UniqueConstraintViolation(_) => {
                        return Err(AppError::AlreadyExists(format!(
                            "权限名称 '{}' 或 权限路径 '{}' 已存在！",
                            insert_payload.name, insert_payload.authority_uri
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
    let paginator = authority::Entity::find().paginate(db, page.page_size);

    let total_pages = paginator.num_pages().await?;
    let authorities_db = paginator.fetch_page(page.page_number - 1).await?;
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
    let authority_db = authority::Entity::find_by_id(patch_payload.id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("找不到该权限".to_string()))?;
    // 更新
    let mut new_authority: authority::ActiveModel = authority_db.into();
    new_authority.name = Set(patch_payload.name.to_owned());
    new_authority.authority_uri = Set(patch_payload.authority_uri.to_owned());
    new_authority.update_time = Set(Utc::now().naive_utc());

    let db_authority = new_authority.update(db).await?;

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
    let res = authority::Entity::delete_by_id(delete_payload.id)
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
#[openapi(paths(delete_authority, patch_authority, add_authority, get_page_authorities))]
pub struct AuthorityApi;
