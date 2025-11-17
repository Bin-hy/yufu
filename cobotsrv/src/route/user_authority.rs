use crate::AppState;
use crate::entity::user_authority::{
    UseAuthorityAddReq, UseAuthorityDeleteReq, UseAuthorityPatchReq, UserAuthorityVO,
};
use crate::entity::{PageSelectQuery, PageSelectQueryResult, user_authority};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use axum::extract::{Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DeleteResult, EntityTrait, PaginatorTrait, QueryFilter, Set,
};

pub fn get_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/user_authority_override", post(add_user_authority))
        .route("/user_authority_override", delete(delete_user_authority))
        .route("/user_authority_override", patch(patch_user_authority))
        .route("/user_authority_override", get(get_user_authorities_page))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/user_authority_override",
    description = "添加覆写条目",
    request_body(
        content = UseAuthorityAddReq,
        description = "添加覆写条目",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "添加成功", body = UserAuthorityVO),
    ),
))]
pub async fn add_user_authority(
    State(state): State<AppState>,
    Json(payload): Json<UseAuthorityAddReq>,
) -> Result<Json<UserAuthorityVO>> {
    let db = &state.database.connection;

    // 校验用户与权限是否存在
    let user_vo = crate::service::user::UserService::get_user_by_id(db, payload.user_id).await?;
    let authority_vo =
        crate::service::authority::Service::get_authority_vo(db, payload.authority_id).await?;

    let now = Utc::now().naive_utc();
    let inserted = user_authority::ActiveModel {
        user_id: Set(payload.user_id),
        authority_id: Set(payload.authority_id),
        allow: Set(payload.allow),
        create_time: Set(now),
        update_time: Set(now),
    }
    .insert(db)
    .await
    .into_app_with_conflict_msg("覆写条目已存在！")?;

    let vo = UserAuthorityVO {
        allow: inserted.allow,
        user_vo: Some(user_vo),
        authority_vo: Some(authority_vo),
        create_time: inserted.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        update_time: inserted.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    Ok(Json(vo))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    delete,
    path = "/user_authority_override",
    description = "删除用户的权限覆写",
    request_body(
        content = UseAuthorityDeleteReq,
        description = "删除用户的权限覆写",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "删除成功", body = String, example = json!("删除成功")),
        (status = 404, description = "删除失败", body = String, example = json!("删除失败,数据库中不存在或者已删除！")),
    ),
))]
pub async fn delete_user_authority(
    State(state): State<AppState>,
    Json(payload): Json<UseAuthorityDeleteReq>,
) -> Result<Json<String>> {
    let db = &state.database.connection;

    // 删除 user_id + authority_id 对应的记录
    let res: DeleteResult = user_authority::Entity::delete_many()
        .filter(user_authority::Column::UserId.eq(payload.user_id))
        .filter(user_authority::Column::AuthorityId.eq(payload.authority_id))
        .exec(db)
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

#[cfg_attr(feature = "openapi", utoipa::path(
    patch,
    path = "/user_authority_override",
    description = "更新用户覆写内容",
    request_body(
        content = UseAuthorityPatchReq,
        description = "更新用户覆写内容",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "更新角色信息", body = UserAuthorityVO),
        (status = 404, description = "找不到该 用户 | 权限"),
    ),
))]
pub async fn patch_user_authority(
    State(state): State<AppState>,
    Json(payload): Json<UseAuthorityPatchReq>,
) -> Result<Json<UserAuthorityVO>> {
    let db = &state.database.connection;

    // 查找现有记录
    let existing = user_authority::Entity::find()
        .filter(user_authority::Column::UserId.eq(payload.user_id))
        .filter(user_authority::Column::AuthorityId.eq(payload.authority_id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("找不到该 用户 | 权限".to_string()))?;

    // 更新 allow 与时间戳
    let mut am: user_authority::ActiveModel = existing.into();
    am.allow = Set(payload.allow);
    am.update_time = Set(Utc::now().naive_utc());
    let updated = am.update(db).await?;

    let user_vo = crate::service::user::UserService::get_user_by_id(db, updated.user_id).await?;
    let authority_vo =
        crate::service::authority::Service::get_authority_vo(db, updated.authority_id).await?;
    let vo = UserAuthorityVO {
        allow: updated.allow,
        user_vo: Some(user_vo),
        authority_vo: Some(authority_vo),
        create_time: updated.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        update_time: updated.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    Ok(Json(vo))
}

#[cfg_attr(feature = "openapi", utoipa::path(
    get,
    path = "/user_authority_override",
    description = "获取覆写权限",
    responses(
        (status = 200, description = "获取成功", body = PageSelectQueryResult<UserAuthorityVO>),
    ),
))]
pub async fn get_user_authorities_page(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<UserAuthorityVO>>> {
    let db = &state.database.connection;

    let page_size = page.page_size;
    let page_number = page.page_number;

    let paginator = user_authority::Entity::find().paginate(db, page_size);
    let total_pages = paginator.num_pages().await?;
    let records = paginator.fetch_page(page_number - 1).await?;

    let mut data_list = Vec::with_capacity(records.len());
    for ua in records {
        // 组装 user_vo 与 authority_vo
        let user_vo = crate::service::user::UserService::get_user_by_id(db, ua.user_id).await?;
        let authority_vo =
            crate::service::authority::Service::get_authority_vo(db, ua.authority_id).await?;

        let vo = UserAuthorityVO {
            allow: ua.allow,
            user_vo: Some(user_vo),
            authority_vo: Some(authority_vo),
            create_time: ua.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            update_time: ua.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        };
        data_list.push(vo);
    }

    Ok(Json(PageSelectQueryResult::<UserAuthorityVO> {
        page_size,
        page_number,
        total_pages,
        data_list,
    }))
}

#[cfg(feature = "openapi")]
use utoipa::OpenApi;

#[cfg(feature = "openapi")]
#[derive(OpenApi)]
#[openapi(paths(
    add_user_authority,
    delete_user_authority,
    patch_user_authority,
    get_user_authorities_page
))]
pub struct UserAuthorityApi;
