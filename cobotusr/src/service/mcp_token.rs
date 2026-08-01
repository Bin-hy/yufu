//! Persistence + lifecycle for revocable MCP access tokens.
//!
//! Each issued token is one row keyed by its `jti`. The table doubles as a revocation
//! denylist: a row with `revoked = true` denies that `jti`. Unknown jtis (ordinary OIDC
//! session tokens) are not present and therefore not revoked.

use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder,
};

use crate::IdType;
use crate::entity::mcp_token::{self, McpTokenVO};

/// Persist a new MCP-token record. The token JWT is signed separately and shares this `jti`.
///
/// `now` is the single timestamp the caller also used to derive `expires_at` (and,
/// immediately after, to sign the JWT), so the row and the token agree on their
/// lifetime. Accepts any connection so refresh can insert inside a transaction.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    db: &impl ConnectionTrait,
    user_id: IdType,
    name: &str,
    jti: &str,
    aud: &str,
    now: chrono::NaiveDateTime,
    expires_at: chrono::NaiveDateTime,
) -> Result<mcp_token::Model, DbErr> {
    mcp_token::ActiveModel {
        id: NotSet,
        user_id: Set(user_id),
        name: Set(name.to_string()),
        jti: Set(jti.to_string()),
        aud: Set(aud.to_string()),
        created_at: Set(now),
        expires_at: Set(expires_at),
        revoked: Set(false),
    }
    .insert(db)
    .await
}

/// The `WHERE` shared by "active token" queries: not revoked and not yet expired.
fn active_filter() -> sea_orm::Condition {
    sea_orm::Condition::all()
        .add(mcp_token::Column::Revoked.eq(false))
        .add(mcp_token::Column::ExpiresAt.gt(chrono::Utc::now().naive_utc()))
}

/// List the caller's active (non-revoked, non-expired) tokens, newest first.
pub async fn list_active_by_user(
    db: &DatabaseConnection,
    user_id: IdType,
) -> Result<Vec<McpTokenVO>, DbErr> {
    let rows = mcp_token::Entity::find()
        .filter(mcp_token::Column::UserId.eq(user_id))
        .filter(active_filter())
        .order_by_desc(mcp_token::Column::CreatedAt)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(McpTokenVO::from).collect())
}

/// Count the caller's active (non-revoked, non-expired) tokens — used to enforce
/// the per-user quota on issue.
pub async fn count_active_by_user(db: &DatabaseConnection, user_id: IdType) -> Result<u64, DbErr> {
    mcp_token::Entity::find()
        .filter(mcp_token::Column::UserId.eq(user_id))
        .filter(active_filter())
        .count(db)
        .await
}

/// Fetch one of the caller's tokens by id (ownership-scoped).
pub async fn find_owned(
    db: &DatabaseConnection,
    user_id: IdType,
    id: IdType,
) -> Result<Option<mcp_token::Model>, DbErr> {
    mcp_token::Entity::find_by_id(id)
        .filter(mcp_token::Column::UserId.eq(user_id))
        .one(db)
        .await
}

/// Rename one of the caller's tokens. Returns `false` if not found / not owned.
pub async fn rename(
    db: &DatabaseConnection,
    user_id: IdType,
    id: IdType,
    name: &str,
) -> Result<bool, DbErr> {
    let Some(row) = find_owned(db, user_id, id).await? else {
        return Ok(false);
    };
    let mut am: mcp_token::ActiveModel = row.into();
    am.name = Set(name.to_string());
    am.update(db).await?;
    Ok(true)
}

/// Revoke (soft-delete) one of the caller's tokens. Returns `false` if not found /
/// not owned. Accepts any connection so refresh can revoke inside a transaction.
pub async fn revoke(db: &impl ConnectionTrait, user_id: IdType, id: IdType) -> Result<bool, DbErr> {
    let Some(row) = mcp_token::Entity::find_by_id(id)
        .filter(mcp_token::Column::UserId.eq(user_id))
        .one(db)
        .await?
    else {
        return Ok(false);
    };
    let mut am: mcp_token::ActiveModel = row.into();
    am.revoked = Set(true);
    am.update(db).await?;
    Ok(true)
}

/// Denylist check used by cobotsrv: is this `jti` an explicitly revoked MCP token?
/// Unknown jtis (ordinary OIDC tokens, never tracked here) return `false`.
pub async fn is_revoked(db: &DatabaseConnection, jti: &str) -> Result<bool, DbErr> {
    let found = mcp_token::Entity::find()
        .filter(mcp_token::Column::Jti.eq(jti))
        .filter(mcp_token::Column::Revoked.eq(true))
        .one(db)
        .await?;
    Ok(found.is_some())
}
