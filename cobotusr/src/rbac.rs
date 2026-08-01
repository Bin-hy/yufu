use crate::entity::{authority, authority_role, user, user_authority};
use crate::error::AppError;
use crate::result::Result;

use crate::service::recording;
use crate::{AppState, parse_user_id};
use auth::claims::AccessTokenClaims;
use axum::Json;
use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

// 目前精细化到query只处理到path例如：get?/user/{user_id}
// 检查每个请求的每个角色是否符合他的角色权限
pub async fn authority_check_middleware_rbac_abac(
    claim: AccessTokenClaims,
    State(app_state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let db = &app_state.database.connection;
    let user_id = match parse_user_id(&claim.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "invalid sub").into_response(),
    };

    // 从 extensions 中提取用户（已由 access_callback 加载）
    let user = match request.extensions().get::<user::Model>() {
        Some(u) => u.clone(),
        None => {
            return AppError::Unauthorized("User not found in request context".to_string())
                .into_response();
        }
    };

    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());

    let method = request.method().to_string().to_uppercase();

    let prefix_api = app_state.config.http.api_prefix.clone();
    let uri = matched_path; //request.uri().to_string();
    let pure_uri = get_pure_uri(prefix_api, uri.clone());

    let role_id = user.role_id;

    // ABAC 优先：如果 authority 未登记，则不做权限管控，直接放行
    let authority_opt = match authority::Entity::find()
        .filter(authority::Column::AuthorityUri.eq(&pure_uri))
        .filter(authority::Column::Method.eq(&method))
        .one(db)
        .await
    {
        Ok(a) => a,
        Err(e) => return AppError::from(e).into_response(),
    };

    if authority_opt.is_none() {
        return next.run(request).await;
    }

    // 存在该权限路径：先检查用户覆写（user_authority），有显式 allow/deny 则遵循覆写
    let authority_id = authority_opt.unwrap().id;
    match user_authority::Entity::find()
        .filter(user_authority::Column::UserId.eq(user_id))
        .filter(user_authority::Column::AuthorityId.eq(authority_id))
        .one(db)
        .await
    {
        Ok(Some(ua)) => {
            if ua.allow {
                return next.run(request).await;
            } else {
                return AppError::Forbidden(format!("{} No access permission (ABAC deny)", uri))
                    .into_response();
            }
        }
        Ok(None) => { /* 无覆写，进入 RBAC */ }
        Err(e) => return AppError::from(e).into_response(),
    }

    // RBAC：检查角色是否拥有该权限
    let exists = authority_role::Entity::find()
        .filter(authority_role::Column::RoleId.eq(role_id))
        .find_also_related(authority::Entity)
        .filter(authority::Column::AuthorityUri.eq(&pure_uri))
        .filter(authority::Column::Method.eq(&method))
        .one(db)
        .await;
    match exists {
        Ok(Some(_)) => {
            // 有权限，继续执行请求
            next.run(request).await
        }
        Ok(e) => {
            // 无权限
            AppError::Forbidden(format!("{} No access permission, {:?}", uri, e)).into_response()
        }
        Err(e) => AppError::from(e).into_response(),
    }
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Clone, Deserialize, Serialize)]
pub struct CheckRequest {
    pub token: Option<String>,
    pub req_path: String,
    pub method: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Clone, Deserialize, Serialize)]
pub struct CheckResponse {
    pub allow: bool,
}

// 1. 查 user_authority 表查看权限是否有复写 allow == true 则 有权限， 否则无权限
// 2. 无复写进行RBAC查询即可
#[cfg_attr(feature = "openapi", utoipa::path(
    post,
    path = "/rbac/abac",
    request_body(
        content = CheckRequest,
        description = "作为lib包进行rbac&abac权限校验",
        content_type = "application/json"
    ),
    description = "检查权限能力（RBAC&ABAC）",
    responses(
        (status = 200, description = "是否具有权限", body = CheckRequest),
    ),
))]
pub async fn check_authority_abac(
    State(state): State<AppState>,
    Json(check_request): Json<CheckRequest>,
) -> Result<Json<CheckResponse>> {
    let db = &state.database.connection;

    let token = check_request.token.clone().unwrap_or("".to_string());
    let method = check_request.method.clone().to_string().to_uppercase();
    let path = check_request.req_path.clone();

    // 构造用于匹配 authority 的 uri（忽略前缀与 query）
    let prefix_api = state.config.http.api_prefix.clone();
    let pure_uri = handler_prefix(prefix_api, path.clone());

    // 如果 authority 表不存在该记录，则不做权限管控，放行
    let authority_opt = authority::Entity::find()
        .filter(authority::Column::AuthorityUri.eq(&pure_uri))
        .filter(authority::Column::Method.eq(&method))
        .one(db)
        .await?;
    if authority_opt.is_none() {
        return Ok(Json(CheckResponse { allow: true }));
    }

    // 校验 token 并获取用户
    let claim = match state.id_token_signer().verify_access_token(&token, None) {
        Ok(claim) => claim,
        Err(_) => return Err(AppError::Forbidden("permission not access...".to_string())),
    };
    let user_id = parse_user_id(&claim.sub)?;

    let user = match user::Entity::find_by_id(user_id).one(db).await {
        Ok(Some(u)) => u,
        Ok(None) => return Err(AppError::NotFound(format!("User {} not found", user_id))),
        Err(e) => return Err(AppError::from(e)),
    };

    let authority_id = authority_opt.unwrap().id;

    // 先检查 ABAC 覆写：用户对该权限是否有显式 allow/deny
    let override_opt = user_authority::Entity::find()
        .filter(user_authority::Column::UserId.eq(user_id))
        .filter(user_authority::Column::AuthorityId.eq(authority_id))
        .one(db)
        .await?;

    if let Some(ua) = override_opt {
        if ua.allow {
            // 显式允许：记录并放行
            recording::Service::add_recording(db, method, path, user.id).await?;
            return Ok(Json(CheckResponse { allow: true }));
        } else {
            // 显式拒绝：直接拒绝
            return Ok(Json(CheckResponse { allow: false }));
        }
    }

    // 无覆写则走 RBAC：检查角色是否拥有该权限
    let exists = authority_role::Entity::find()
        .filter(authority_role::Column::RoleId.eq(user.role_id))
        .find_also_related(authority::Entity)
        .filter(authority::Column::AuthorityUri.eq(&pure_uri))
        .one(db)
        .await;

    match exists {
        Ok(Some(_)) => {
            // 有权限, 放行, 加入记录
            recording::Service::add_recording(db, method, path, user.id).await?;
            Ok(Json(CheckResponse { allow: true }))
        }
        Ok(_) => {
            // 无权限
            Ok(Json(CheckResponse { allow: false }))
        }
        Err(e) => Err(AppError::from(e)),
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

// 清除 path中的 query
pub fn handle_query_path(uri: String) -> String {
    uri.split("?").next().unwrap_or(&uri).to_string()
}

// 获取纯净的 uri无前缀和query参数
pub fn get_pure_uri(prefix: String, uri: String) -> String {
    let uri_no_prefix = handler_prefix(prefix, uri);
    handle_query_path(uri_no_prefix)
}
