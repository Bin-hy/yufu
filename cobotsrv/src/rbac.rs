// 处理权限查询

use crate::AppState;
use crate::entity::role::RoleEnum;
use crate::entity::{authority, authority_role, user};
use crate::error::AppError;
use auth::claims::Claims;
use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

// 目前精细化到query只处理到path例如：get?/user/{user_id}
// 检查每个请求的每个角色是否符合他的角色权限
pub async fn authority_check_middleware(
    claim: Claims,
    State(app_state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let db = &app_state.database.connection;
    let user_id = claim.id;
    // 检查用户的角色role是否有请求该路径的权限
    // 检索role
    // 查找用户，不存在则返回自定义错误
    let user = match user::Entity::find_by_id(&user_id).one(db).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return AppError::NotFound(format!("User {} not found", &user_id)).into_response();
        }
        Err(e) => {
            return AppError::from(e).into_response();
        }
    };

    // 超级管理员一切放行
    if user.role_id == RoleEnum::Superadmin.default_id() {
        return next.run(request).await;
    }

    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());

    let prefix_api = app_state.config.http.api_prefix.clone();
    let uri = matched_path; //request.uri().to_string();
    let _query = request.uri().query().unwrap_or("");

    let uri_no_prefix = handler_prefix(prefix_api, uri.clone());

    println!(
        "{}?{}",
        request.method().clone().as_str().to_lowercase(),
        uri_no_prefix.clone()
    );

    let authority_uri_no_prefix = format!(
        "{}?{}",
        request.method().as_str().to_lowercase(),
        uri_no_prefix
    );

    let role_id = user.role_id;

    let exists = authority_role::Entity::find()
        .filter(authority_role::Column::RoleId.eq(&role_id))
        .find_also_related(authority::Entity)
        .filter(authority::Column::Authority.eq(&authority_uri_no_prefix))
        .one(db)
        .await;
    match exists {
        Ok(Some(_)) => {
            // 有权限，继续执行请求
            next.run(request).await
        }
        Ok(e) => {
            // 无权限
            AppError::Forbidden(format!("{} 无访问权限,{:?}", &uri, e)).into_response()
        }
        Err(e) => AppError::from(e).into_response(),
    }
}

// 检查 uri 是否以 prefix 开头
fn handler_prefix(prefix: String, uri: String) -> String {
    if uri.starts_with(&prefix) {
        uri[prefix.len()..].to_string()
    } else {
        uri
    }
}
