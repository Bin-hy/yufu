//! The compatibility table supplies labels; registry and grant supply security facts.
use super::*;
use crate::entity::mcp_token;
use sea_orm::QueryOrder;

fn user_id(principal: &PrincipalRef) -> Result<crate::IdType, SecurityError> {
    let id = principal
        .subject()
        .parse::<crate::IdType>()
        .map_err(|_| SecurityError::InvalidCredential)?;
    if principal.kind() != PrincipalKind::User || id <= 0 || id.to_string() != principal.subject() {
        return Err(SecurityError::InvalidCredential);
    }
    Ok(id)
}
fn millis(seconds: u64) -> Result<i64, SecurityError> {
    seconds
        .checked_mul(1000)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or(SecurityError::InvalidCredential)
}
fn datetime(seconds: u64) -> Result<chrono::NaiveDateTime, SecurityError> {
    chrono::DateTime::from_timestamp(
        i64::try_from(seconds).map_err(|_| SecurityError::InvalidCredential)?,
        0,
    )
    .map(|d| d.naive_utc())
    .ok_or(SecurityError::InvalidCredential)
}
impl PgSecurityTransaction {
    pub(super) async fn check_mcp_quota(
        &self,
        principal: &PrincipalRef,
    ) -> Result<(), SecurityError> {
        let row = self.run(self.transaction.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT count(DISTINCT g.id) AS active FROM security_credential c JOIN security_grant g ON g.id=c.grant_id WHERE c.issuer=$1 AND c.principal_kind='user' AND c.subject=$2 AND c.token_use='mcp_access' AND c.credential_kind='access' AND c.revoked_at IS NULL AND c.consumed_at IS NULL AND c.expires_at>$3 AND g.revoked_at IS NULL AND g.grant_kind='mcp'",
            [principal.issuer().into(), principal.subject().into(), i64::try_from(now(self.clock.as_ref())?).map_err(|_| SecurityError::DependencyUnavailable)?.into()]))).await?
            .ok_or(SecurityError::DependencyUnavailable)?;
        if row.try_get::<i64>("", "active").map_err(unavailable)? >= 20 {
            return Err(SecurityError::CapacityExceeded);
        }
        Ok(())
    }
    pub(super) async fn insert_mcp(
        &mut self,
        r: McpRegistration,
    ) -> Result<mcp_token::Model, SecurityError> {
        let c = r.claims;
        let principal = PrincipalRef::new(&c.iss, c.principal_kind, &c.sub)?;
        let user_id = user_id(&principal)?;
        let grant_id: Uuid = c
            .grant_id
            .parse()
            .map_err(|_| SecurityError::InvalidCredential)?;
        if c.token_use != TokenUse::McpAccess
            || c.application != "cobotsrv"
            || c.aud != c.application
            || c.session_id.is_some()
            || c.family_id.is_some()
            || c.binding != IdentityBinding::None
        {
            return Err(SecurityError::InvalidCredential);
        }
        if r.new_root {
            self.run(
                grant::ActiveModel {
                    id: Set(grant_id),
                    issuer: Set(c.iss.clone()),
                    principal_kind: Set("user".into()),
                    subject: Set(c.sub.clone()),
                    application: Set(c.application.clone()),
                    grant_kind: Set("mcp".into()),
                    created_at: Set(millis(c.iat)?),
                    revoked_at: Set(None),
                }
                .insert(&self.transaction),
            )
            .await?;
        }
        self.run(
            credential::ActiveModel {
                issuer: Set(c.iss.clone()),
                jti: Set(c.jti.clone()),
                principal_kind: Set("user".into()),
                subject: Set(c.sub),
                credential_kind: Set("access".into()),
                token_use: Set("mcp_access".into()),
                audience: Set(c.aud.clone()),
                application: Set(c.application),
                bounds: Set(
                    serde_json::to_value(c.bounds).map_err(|_| SecurityError::InvalidCredential)?
                ),
                binding: Set(serde_json::to_value(c.binding)
                    .map_err(|_| SecurityError::InvalidCredential)?),
                grant_id: Set(grant_id),
                session_id: Set(None),
                family_id: Set(None),
                paired_access_jti: Set(None),
                oidc_client_id: Set(None),
                revocation_parent_jti: Set(None),
                minted_by: Set(Some(
                    serde_json::json!({"issuer":r.minted_by.issuer(),"jti":r.minted_by.jti()}),
                )),
                signing_key_id: Set(Some(r.key_id)),
                secret_digest: Set(None),
                issued_at: Set(millis(c.iat)?),
                not_before: Set(None),
                expires_at: Set(millis(c.exp)?),
                revoked_at: Set(None),
                consumed_at: Set(None),
                replacement_jti: Set(None),
            }
            .insert(&self.transaction),
        )
        .await?;
        self.run(
            mcp_token::ActiveModel {
                id: sea_orm::NotSet,
                user_id: Set(user_id),
                name: Set(r.name),
                jti: Set(c.jti),
                aud: Set(c.aud),
                created_at: Set(datetime(c.iat)?),
                expires_at: Set(datetime(c.exp)?),
                revoked: Set(false),
            }
            .insert(&self.transaction),
        )
        .await
    }
    pub(super) async fn owned_mcp_metadata(
        &self,
        principal: &PrincipalRef,
        id: crate::IdType,
    ) -> Result<Option<mcp_token::Model>, SecurityError> {
        self.run(
            mcp_token::Entity::find_by_id(id)
                .filter(mcp_token::Column::UserId.eq(user_id(principal)?))
                .one(&self.transaction),
        )
        .await
    }
    pub(super) async fn consume_mcp_credential(
        &self,
        old: &CredentialRef,
        at: u64,
    ) -> Result<(), SecurityError> {
        let at = i64::try_from(at).map_err(|_| SecurityError::DependencyUnavailable)?;
        let result = self.run(self.transaction.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            // Access rows use conditional revocation as their one-time spend.
            // consumed_at/replacement_jti are reserved for refresh replay history.
            "UPDATE security_credential SET revoked_at=$3 WHERE issuer=$1 AND jti=$2 AND credential_kind='access' AND token_use='mcp_access' AND revoked_at IS NULL AND expires_at>$3",
            [old.issuer().into(),old.jti().into(),at.into()]))).await?;
        if result.rows_affected() != 1 {
            return Err(SecurityError::Conflict);
        }
        Ok(())
    }
    pub(super) async fn active_mcp_metadata(
        &mut self,
        principal: &PrincipalRef,
    ) -> Result<Vec<mcp_token::Model>, SecurityError> {
        let rows = self
            .run(
                mcp_token::Entity::find()
                    .filter(mcp_token::Column::UserId.eq(user_id(principal)?))
                    .filter(mcp_token::Column::Revoked.eq(false))
                    .order_by_desc(mcp_token::Column::CreatedAt)
                    .all(&self.transaction),
            )
            .await?;
        let mut active = Vec::new();
        for row in rows {
            let reference = CredentialRef::new(principal.issuer(), &row.jti)?;
            let Some(snapshot) = self.credential(&reference).await? else {
                continue;
            };
            if super::super::lifecycle::status_for_snapshot(&snapshot, now(self.clock.as_ref())?)
                .is_ok()
                && snapshot.current.credential.token_use == "mcp_access"
                && snapshot.current.credential.subject == principal.subject()
                && snapshot.current.credential.application == row.aud
                && snapshot.current.credential.expires_at
                    == row.expires_at.and_utc().timestamp_millis()
            {
                active.push(row);
            }
        }
        Ok(active)
    }
    pub(super) async fn rename_owned_mcp(
        &self,
        principal: &PrincipalRef,
        id: crate::IdType,
        name: &str,
    ) -> Result<(), SecurityError> {
        let result = self
            .run(
                mcp_token::Entity::update_many()
                    .col_expr(
                        mcp_token::Column::Name,
                        sea_orm::sea_query::Expr::value(name),
                    )
                    .filter(mcp_token::Column::Id.eq(id))
                    .filter(mcp_token::Column::UserId.eq(user_id(principal)?))
                    .exec(&self.transaction),
            )
            .await?;
        if result.rows_affected != 1 {
            return Err(SecurityError::Forbidden);
        }
        Ok(())
    }
}
