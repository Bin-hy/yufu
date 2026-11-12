//! Legacy MCP denylist adapter. Creation, rotation, metadata management and
//! revocation are performed by the primary registry lifecycle transaction.
use crate::entity::mcp_token;
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};

/// Compatibility information only: an unknown reference is not proof of liveness.
pub async fn is_revoked(db: &DatabaseConnection, jti: &str) -> Result<bool, DbErr> {
    let found = mcp_token::Entity::find()
        .filter(mcp_token::Column::Jti.eq(jti))
        .filter(mcp_token::Column::Revoked.eq(true))
        .one(db)
        .await?;
    Ok(found.is_some())
}
