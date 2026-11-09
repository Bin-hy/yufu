use super::*;
use crate::service::security::revocation::StoredRevocationScope;

impl PgSecurityTransaction {
    pub(super) async fn serialize_user_roots(
        &self,
        principal: &PrincipalRef,
    ) -> Result<(), SecurityError> {
        if principal.kind() != PrincipalKind::User {
            return Err(SecurityError::Forbidden);
        }
        let id = principal
            .subject()
            .parse::<crate::IdType>()
            .map_err(|_| SecurityError::InvalidResource)?;
        if id <= 0 || id.to_string() != principal.subject() {
            return Err(SecurityError::InvalidResource);
        }
        // A write barrier makes a stale REPEATABLE READ mint fail with 40001
        // after a concurrent application revocation or administrative logout.
        self.acquire_identity_scope(
            &crate::service::identity::repository::IdentityLockScope::new(vec![
                crate::service::identity::repository::IdentityLockKey::User(id),
            ])?,
        )
        .await
        .map_err(Into::into)
    }
    pub(super) async fn lock_revocation_credential(
        &mut self,
        reference: &CredentialRef,
    ) -> Result<Option<CredentialSnapshot>, SecurityError> {
        let Some(snapshot) = self.credential(reference).await? else {
            return Ok(None);
        };
        let row = &snapshot.current.credential;
        self.lock_identity_credential(row).await?;
        self.credential(reference).await
    }
    pub(super) async fn resolve_consent_application(
        &self,
        principal: &PrincipalRef,
        consent_id: i32,
    ) -> Result<String, SecurityError> {
        let user_id = principal
            .subject()
            .parse::<crate::IdType>()
            .map_err(|_| SecurityError::InvalidCredential)?;
        let row = self
            .run(
                self.transaction
                    .query_one_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT client_id FROM oauth_consent WHERE id=$1 AND user_id=$2 FOR UPDATE",
                        [consent_id.into(), user_id.into()],
                    )),
            )
            .await?
            .ok_or(SecurityError::Forbidden)?;
        let client_id: String = row.try_get("", "client_id").map_err(unavailable)?;
        let rows=self.run(self.transaction.query_all_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT application FROM security_session_client WHERE issuer=$1 AND client_id=$2 UNION SELECT application FROM security_credential WHERE issuer=$1 AND oidc_client_id=$2 AND principal_kind='user' AND subject=$3",
            [principal.issuer().into(),client_id.into(),principal.subject().into()]))).await?;
        if rows.len() != 1 {
            return Err(SecurityError::DependencyUnavailable);
        }
        let app: String = rows[0].try_get("", "application").map_err(unavailable)?;
        validate_application(&app)?;
        Ok(app)
    }
    pub(super) async fn revoke_stored_scope(
        &self,
        principal: &PrincipalRef,
        scope: &StoredRevocationScope,
        at: u64,
    ) -> Result<(), SecurityError> {
        let at = i64::try_from(at).map_err(|_| SecurityError::DependencyUnavailable)?;
        let (predicate, target): (&str, sea_orm::Value) = match scope {
            StoredRevocationScope::Credential(reference) => ("c.jti=$4", reference.jti().into()),
            StoredRevocationScope::CurrentSession(id) => ("c.session_id=$4", (*id).into()),
            StoredRevocationScope::RefreshFamily(id) => ("c.family_id=$4", (*id).into()),
            StoredRevocationScope::ApplicationGrants(app) => {
                validate_application(app)?;
                self.serialize_user_roots(principal).await?;
                ("c.application=$4", app.as_str().into())
            }
            StoredRevocationScope::UserSessions => {
                self.serialize_user_roots(principal).await?;
                (
                    "g.grant_kind='session' AND c.principal_kind=$4",
                    "user".into(),
                )
            }
        };
        if let StoredRevocationScope::Credential(reference) = scope
            && reference.issuer() != principal.issuer()
        {
            return Err(SecurityError::Forbidden);
        }
        let parameters = vec![
            principal.issuer().into(),
            serde_json::to_value(principal.kind())
                .map_err(|_| SecurityError::InvalidCredential)?
                .as_str()
                .ok_or(SecurityError::InvalidCredential)?
                .into(),
            principal.subject().into(),
            target,
        ];
        // Grant locks use the same order as refresh rotation. Application and
        // administrative scopes also serialize against creation of new roots.
        let roots=self.run(self.transaction.query_all_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            format!("SELECT g.id FROM security_grant g WHERE g.issuer=$1 AND g.principal_kind=$2 AND g.subject=$3 AND {} ORDER BY g.id FOR UPDATE", match scope {
                StoredRevocationScope::ApplicationGrants(_) => "g.application=$4".to_owned(),
                StoredRevocationScope::UserSessions => "g.grant_kind='session' AND g.principal_kind=$4".to_owned(),
                _ => format!("g.id IN (SELECT c.grant_id FROM security_credential c WHERE c.issuer=$1 AND c.principal_kind=$2 AND c.subject=$3 AND {predicate})"),
            }),parameters.clone()))).await?;
        if roots.is_empty()
            && !matches!(
                scope,
                StoredRevocationScope::ApplicationGrants(_) | StoredRevocationScope::UserSessions
            )
        {
            return Err(SecurityError::Forbidden);
        }
        let ids: Vec<Uuid> = roots
            .iter()
            .map(|row| row.try_get("", "id").map_err(unavailable))
            .collect::<Result<_, _>>()?;
        for id in &ids {
            if matches!(scope, StoredRevocationScope::ApplicationGrants(_)) {
                self.run(self.transaction.execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "UPDATE security_grant SET revoked_at=COALESCE(revoked_at,$2) WHERE id=$1",
                    [(*id).into(), at.into()],
                )))
                .await?;
            }
        }
        let credential_filter =
            format!("c.issuer=$1 AND c.principal_kind=$2 AND c.subject=$3 AND {predicate}");
        if matches!(
            scope,
            StoredRevocationScope::CurrentSession(_)
                | StoredRevocationScope::ApplicationGrants(_)
                | StoredRevocationScope::UserSessions
        ) {
            self.run(self.transaction.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                format!("UPDATE security_session SET revoked_at=COALESCE(revoked_at,$5) WHERE issuer=$1 AND principal_kind=$2 AND subject=$3 AND id IN (SELECT c.session_id FROM security_credential c JOIN security_grant g ON g.id=c.grant_id WHERE {credential_filter})"),
                parameters.iter().cloned().chain([at.into()]).collect::<Vec<_>>()))).await?;
        }
        if !matches!(scope, StoredRevocationScope::Credential(_)) {
            self.run(self.transaction.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                format!("UPDATE security_refresh_family SET revoked_at=COALESCE(revoked_at,$5) WHERE issuer=$1 AND principal_kind=$2 AND subject=$3 AND id IN (SELECT c.family_id FROM security_credential c JOIN security_grant g ON g.id=c.grant_id WHERE {credential_filter})"),
                parameters.iter().cloned().chain([at.into()]).collect::<Vec<_>>()))).await?;
        }
        self.run(self.transaction.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            format!("WITH RECURSIVE descendants(jti) AS (SELECT c.jti FROM security_credential c JOIN security_grant g ON g.id=c.grant_id WHERE {credential_filter} UNION SELECT c.jti FROM security_credential c JOIN descendants d ON c.revocation_parent_jti=d.jti WHERE c.issuer=$1) UPDATE security_credential SET revoked_at=COALESCE(revoked_at,$5) WHERE issuer=$1 AND jti IN (SELECT jti FROM descendants)"),
            parameters.iter().cloned().chain([at.into()]).collect::<Vec<_>>()))).await?;
        // Compatibility labels mirror committed revocation; they never supply
        // registry liveness or turn an unknown old JWT into a trusted credential.
        self.run(self.transaction.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "UPDATE mcp_token m SET revoked=TRUE FROM security_credential c WHERE c.issuer=$1 AND c.principal_kind=$2 AND c.subject=$3 AND c.token_use='mcp_access' AND c.revoked_at IS NOT NULL AND m.jti=c.jti AND m.aud=c.application AND m.user_id::text=c.subject",
            parameters[..3].to_vec()))).await?;
        if let StoredRevocationScope::ApplicationGrants(app) = scope {
            let user_id = principal
                .subject()
                .parse::<crate::IdType>()
                .map_err(|_| SecurityError::InvalidResource)?;
            // Cancel unexchanged codes and all consent entries for the same
            // application, so an old code cannot recreate a revoked grant.
            let clients = "SELECT client_id FROM security_session_client WHERE issuer=$1 AND application=$2 UNION SELECT oidc_client_id FROM security_credential WHERE issuer=$1 AND application=$2 AND principal_kind='user' AND subject=$3 AND oidc_client_id IS NOT NULL";
            let params = vec![
                principal.issuer().into(),
                app.as_str().into(),
                principal.subject().into(),
                user_id.into(),
            ];
            for table in ["oauth_authorization_code", "oauth_consent"] {
                self.run(self.transaction.execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    format!("DELETE FROM {table} WHERE user_id=$4 AND client_id IN ({clients})"),
                    params.clone(),
                )))
                .await?;
            }
            self.run(self.transaction.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                format!("UPDATE oauth_token SET revoked=TRUE WHERE user_id=$4 AND client_id IN ({clients})"),params))).await?;
        }
        Ok(())
    }
}
