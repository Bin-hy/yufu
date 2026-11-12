use crate::entity::{authority, recording};
use crate::error::AppError;
use crate::{AppState, parse_user_id, redact_access_token};
use auth::claims::AccessTokenClaims;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use http::StatusCode;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, Set};

/// 记录用户操作
pub async fn recording_operation_middleware(
    claim: AccessTokenClaims,
    State(app_state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let db = &app_state.database.connection;
    // 获取到用户的操作 get user operations.
    // 查找权限表记录该操作 find the authority in the table.
    let request_method = request.method().to_string().to_uppercase();
    // Identity endpoint queries are removed entirely, including proofs/state.
    let request_full_uri = redact_access_token(request.uri());

    // ignore query_path
    let authority_uri = request_full_uri
        .clone()
        .split("?")
        .next()
        .unwrap_or(&request_full_uri)
        .to_string();

    let (operation_name, operation_uri) = match authority::Entity::find()
        .filter(authority::Column::Method.eq(request_method.clone()))
        .filter(authority::Column::AuthorityUri.eq(&authority_uri))
        .one(db)
        .await
    {
        Ok(Some(authority_obj)) => (authority_obj.name, request_full_uri),
        Ok(None) => return next.run(request).await, // 现在的策略是为 Unknown 说明无需记录
        // Ok(None) => ("Unknown Request...".to_string(), request_full_uri),
        Err(e) => return AppError::from(e).into_response(),
    };

    // claim.sub is a String per OIDC; parse to local IdType
    let user_id = match parse_user_id(&claim.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "invalid sub").into_response(),
    };

    // save into db.
    let new_recording = recording::ActiveModel {
        id: ActiveValue::NotSet,
        name: Set(operation_name.to_owned()),
        method: Set(request_method.to_owned()),
        authority_uri: Set(operation_uri.to_owned()),
        user_id: Set(user_id),
        create_time: Set(Utc::now().naive_utc().to_owned()),
    };
    if let Err(e) = new_recording.insert(db).await {
        return AppError::from(e).into_response();
    }

    next.run(request).await
}
