use crate::IdType;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A long-lived, revocable MCP access token issued via `POST /api/v1/oidc/mcp-tokens`.
///
/// The row carries the token's `jti`; the token value itself is never stored. Setting
/// `revoked = true` denylists that `jti` — cobotsrv checks this on every MCP request, so
/// deleting (or rotating) a token takes effect within the resource server's cache TTL.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "mcp_token")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    /// Owner (JWT `sub`).
    pub user_id: IdType,
    /// User-facing label, editable via rename.
    pub name: String,
    /// The issued token's JWT id (the revocation key).
    pub jti: String,
    /// Audience the token is scoped to.
    pub aud: String,
    pub created_at: DateTime,
    pub expires_at: DateTime,
    pub revoked: bool,
}

/// Metadata view returned to the owner — never includes the token value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpTokenVO {
    pub id: IdType,
    pub name: String,
    pub aud: String,
    pub created_at: String,
    pub expires_at: String,
    pub revoked: bool,
}

impl From<Model> for McpTokenVO {
    fn from(m: Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            aud: m.aud,
            created_at: m.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            expires_at: m.expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            revoked: m.revoked,
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}
