use crate::AppState;
use crate::entity::{authority, recording};
use crate::error::AppError;
use auth::claims::Claims;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// 记录用户操作
pub async fn recording_operation_middleware(
    claim: Claims,
    State(app_state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let db = &app_state.database.connection;
    // 获取到用户的操作 get user operations.
    // 查找权限表记录该操作 find the authority in the table.

    let authority_req = format!(
        "{}?{}",
        request.method().to_string().to_lowercase(),
        request.uri()
    );
    let (operation_name, operation_uri) =
        match authority::Entity::find_by_unique_authority(authority_req.clone())
            .one(db)
            .await
        {
            Ok(Some(authority_obj)) => (authority_obj.name, authority_obj.authority_uri),
            Ok(None) => ("Unknown Request...".to_string(), authority_req),
            Err(e) => return AppError::from(e).into_response(),
        };

    // save into db.
    let new_recording = recording::ActiveModel {
        id: Set(Uuid::new_v4().to_string().to_owned()),
        name: Set(operation_name.to_owned()),
        authority_uri: Set(operation_uri.to_owned()),
        user_id: Set(claim.id.to_owned()),
        create_time: Set(Utc::now().naive_utc().to_owned()),
    };
    if let Err(e) = new_recording.insert(db).await {
        return AppError::from(e).into_response();
    }

    next.run(request).await
}
