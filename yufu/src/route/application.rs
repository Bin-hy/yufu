use crate::AppState;
use crate::entity::application::{
    self, ApplicationDeleteReq, ApplicationInsertReq, ApplicationPatchReq, ApplicationVO,
};
use crate::entity::{PageSelectQuery, PageSelectQueryResult};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use crate::service::identity::legacy_governance::{MutationScope, mutate};
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(insert_payload): Json<ApplicationInsertReq>,
) -> Result<Json<ApplicationVO>> {
    mutate(
        &state,
        &context,
        "POST",
        "/application",
        MutationScope::Registry,
        move |db| {
            let insert_payload = insert_payload.clone();
            Box::pin(async move {
                if insert_payload.oidc_client_id.is_some() {
                    return Err(AppError::BadRequest(
                        "请通过开发者控制台或管理员接入注册身份应用".into(),
                    ));
                }

                let new_app = application::Entity::insert_one(db, insert_payload)
                    .await
                    .into_app_with_conflict_msg("应用标识已存在")?;
                Ok(Json(new_app.into()))
            })
        },
    )
    .await
}

pub async fn get_page_applications(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Query(page): Query<PageSelectQuery>,
) -> Result<Json<PageSelectQueryResult<ApplicationVO>>> {
    crate::service::identity::legacy_governance::require_directory_administrator(&state, &context)
        .await?;
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
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(patch_payload): Json<ApplicationPatchReq>,
) -> Result<Json<ApplicationVO>> {
    mutate(
        &state,
        &context,
        "PATCH",
        "/application",
        MutationScope::Application {
            id: patch_payload.id,
            affects_policy: false,
        },
        move |db| {
            let patch_payload = patch_payload.clone();
            Box::pin(async move {
                if patch_payload.oidc_client_id.is_some() {
                    return Err(AppError::BadRequest("请通过显式纳管流程关联客户端".into()));
                }

                use sea_orm::{ConnectionTrait, DbBackend, Statement};
                let row = db
                    .query_one_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT management_state FROM application WHERE id=$1 FOR UPDATE",
                        [patch_payload.id.into()],
                    ))
                    .await?
                    .ok_or_else(|| AppError::NotFound("应用不存在".into()))?;
                if row.try_get::<String>("", "management_state")? != "legacy_unmanaged" {
                    return Err(AppError::BadRequest(
                        "请通过受控应用治理接口修改或停用应用".into(),
                    ));
                }

                let db_app = application::Entity::update_partial(db, patch_payload).await?;
                Ok(Json(db_app.into()))
            })
        },
    )
    .await
}

pub async fn delete_application(
    State(state): State<AppState>,
    axum::Extension(context): axum::Extension<
        crate::service::identity::legacy_governance::LegacyRequest,
    >,
    Json(delete_payload): Json<ApplicationDeleteReq>,
) -> Result<Json<String>> {
    mutate(
        &state,
        &context,
        "DELETE",
        "/application",
        MutationScope::Application {
            id: delete_payload.id,
            affects_policy: true,
        },
        move |db| {
            let delete_payload = delete_payload.clone();
            Box::pin(async move {
                use sea_orm::{ConnectionTrait, DbBackend, Statement};
                let row = db
                    .query_one_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT management_state FROM application WHERE id=$1 FOR UPDATE",
                        [delete_payload.id.into()],
                    ))
                    .await?
                    .ok_or_else(|| AppError::NotFound("应用不存在".into()))?;
                if row.try_get::<String>("", "management_state")? != "legacy_unmanaged" {
                    return Err(AppError::BadRequest(
                        "请通过受控应用治理接口修改或停用应用".into(),
                    ));
                }

                let ok = application::Entity::delete_checked(db, delete_payload.id).await?;
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
