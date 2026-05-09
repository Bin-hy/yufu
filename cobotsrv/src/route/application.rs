use crate::AppState;
use crate::entity::application::{
    self, ApplicationDeleteReq, ApplicationInsertReq, ApplicationPatchReq, ApplicationVO,
};
use crate::entity::{PageSelectQuery, PageSelectQueryResult};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::routing::post;

pub fn get_auth_routes() -> Router<AppState> {
    Router::new().route(
        "/application",
        post(add_application)
            .get(get_page_applications)
            .patch(patch_application)
            .delete(delete_application),
    )
}

pub async fn add_application(
    State(state): State<AppState>,
    Json(insert_payload): Json<ApplicationInsertReq>,
) -> Result<Json<ApplicationVO>> {
    let db = &state.database.connection;
    let new_app = application::Entity::insert_one(db, insert_payload)
        .await
        .into_app_with_conflict_msg("应用标识已存在")?;
    Ok(Json(new_app.into()))
}

pub async fn get_page_applications(
    State(state): State<AppState>,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<ApplicationVO>>> {
    let db = &state.database.connection;
    let (total_pages, records) =
        application::Entity::paginate(db, page.page_size, page.page_number).await?;
    Ok(Json(PageSelectQueryResult {
        page_size: page.page_size,
        page_number: page.page_number,
        total_pages,
        data_list: records.into_iter().map(ApplicationVO::from).collect(),
    }))
}

pub async fn patch_application(
    State(state): State<AppState>,
    Json(patch_payload): Json<ApplicationPatchReq>,
) -> Result<Json<ApplicationVO>> {
    let db = &state.database.connection;
    let db_app = application::Entity::update_partial(db, patch_payload).await?;
    Ok(Json(db_app.into()))
}

pub async fn delete_application(
    State(state): State<AppState>,
    Json(delete_payload): Json<ApplicationDeleteReq>,
) -> Result<Json<String>> {
    let db = &state.database.connection;
    let ok = application::Entity::delete_checked(db, delete_payload.id).await?;
    if ok {
        Ok(Json("删除成功".to_string()))
    } else {
        Err(AppError::NotFound(
            "删除失败,数据库中不存在或者已删除！".to_string(),
        ))
    }
}
