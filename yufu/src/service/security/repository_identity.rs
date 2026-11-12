use super::*;
use crate::domain::identity::{application::IdentityScopeSet, session::IdentityTokenContext};
use crate::entity::{identity_oidc_rp_session, identity_sso_session};
use crate::service::identity::repository::AdmissionQuery;
use crate::service::identity::repository::pg::{root_snapshot, unsigned};
use crate::service::identity::repository::{IdentityLockKey, IdentityLockScope};

impl PgSecurityTransaction {
    pub(super) async fn lock_identity_credential(
        &self,
        row: &credential::Model,
    ) -> Result<(), SecurityError> {
        let session = match row.session_id {
            Some(id) => {
                self.run(session::Entity::find_by_id(id).one(&self.transaction))
                    .await?
            }
            None => None,
        };
        if session.as_ref().is_none_or(|s| s.sso_session_id.is_none()) {
            // Explicit legacy roots retain the existing grant/session/family
            // contract. In particular, checking an interactive manager must
            // not take the later user creation barrier before signing.
            for (table, id) in [
                ("security_grant", Some(row.grant_id)),
                ("security_session", row.session_id),
                ("security_refresh_family", row.family_id),
            ] {
                if let Some(id) = id {
                    self.run(
                        self.transaction
                            .query_one_raw(Statement::from_sql_and_values(
                                DbBackend::Postgres,
                                format!("SELECT id FROM {table} WHERE id=$1 FOR UPDATE"),
                                [id.into()],
                            )),
                    )
                    .await?;
                }
            }
            self.run(
                self.transaction
                    .query_one_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT jti FROM security_credential WHERE issuer=$1 AND jti=$2 FOR UPDATE",
                        [row.issuer.clone().into(), row.jti.clone().into()],
                    )),
            )
            .await?;
            return Ok(());
        }
        let mut keys = vec![
            IdentityLockKey::Grant(row.grant_id),
            IdentityLockKey::Credential {
                issuer: row.issuer.clone(),
                jti: row.jti.clone(),
            },
        ];
        if let Some(jti) = &row.paired_access_jti {
            keys.push(IdentityLockKey::Credential {
                issuer: row.issuer.clone(),
                jti: jti.clone(),
            });
        }
        if row.principal_kind == "user" {
            keys.push(IdentityLockKey::User(
                row.subject
                    .parse::<i32>()
                    .map_err(|_| SecurityError::InvalidCredential)?,
            ));
        }
        if let Some(id) = row.session_id {
            keys.push(IdentityLockKey::SecuritySession(id));
            let session = session.as_ref().ok_or(SecurityError::InvalidCredential)?;
            if let Some(root) = session.sso_session_id {
                keys.push(IdentityLockKey::Center(root));
            }
            if let Some(rp) = session.oidc_rp_session_id {
                keys.push(IdentityLockKey::RelyingParty(rp));
            }
            if let Some(environment) = session.environment_id {
                keys.push(IdentityLockKey::Environment(environment));
                let env = self
                    .identity_one(
                        "SELECT application_id FROM identity_app_environment WHERE id=$1",
                        vec![environment.into()],
                    )
                    .await?;
                let app: i32 =
                    crate::service::identity::repository::pg::get(&env, "application_id")?;
                keys.push(IdentityLockKey::Application(app));
                let groups=self.identity_rows("SELECT group_id FROM identity_app_assignment WHERE application_id=$1 AND group_id IS NOT NULL ORDER BY group_id",vec![app.into()]).await?;
                for group in groups {
                    keys.push(IdentityLockKey::Group(
                        crate::service::identity::repository::pg::get(&group, "group_id")?,
                    ));
                }
                let owner=self.identity_one("SELECT user_id FROM identity_app_member WHERE application_id=$1 AND member_role='owner'",vec![app.into()]).await?;
                keys.push(IdentityLockKey::User(
                    crate::service::identity::repository::pg::get(&owner, "user_id")?,
                ));
            }
        }
        if let Some(id) = row.family_id {
            keys.push(IdentityLockKey::Family(id));
        }
        self.acquire_identity_scope(&IdentityLockScope::new(keys)?)
            .await
            .map_err(Into::into)
    }
    pub(super) async fn identity_session_state(
        &self,
        session: &session::Model,
        credential: &credential::Model,
    ) -> Result<Option<IdentitySessionState>, SecurityError> {
        let Some(root_id) = session.sso_session_id else {
            if session.environment_id.is_some()
                || session.oidc_rp_session_id.is_some()
                || session.auth_epoch.is_some()
            {
                return Err(SecurityError::InvalidCredential);
            }
            if let Some(client_id) = &credential.oidc_client_id {
                let client = self
                    .run(
                        oauth_client::Entity::find()
                            .filter(oauth_client::Column::ClientId.eq(client_id))
                            .one(&self.transaction),
                    )
                    .await?
                    .ok_or(SecurityError::InvalidCredential)?;
                if client.management_state != "legacy_unmanaged" {
                    return Err(SecurityError::CredentialRevoked);
                }
            }
            if credential.principal_kind == "user"
                && self
                    .run(crate::entity::identity_provider::Entity::find().one(&self.transaction))
                    .await?
                    .is_some()
            {
                let id = credential
                    .subject
                    .parse::<i32>()
                    .map_err(|_| SecurityError::InvalidCredential)?;
                let account = self
                    .identity_account(id)
                    .await?
                    .ok_or(SecurityError::CredentialRevoked)?;
                if !account.enabled {
                    return Err(SecurityError::CredentialRevoked);
                }
            }
            return Ok(None);
        };
        let root = self
            .run(identity_sso_session::Entity::find_by_id(root_id).one(&self.transaction))
            .await?
            .ok_or(SecurityError::CredentialRevoked)?;
        let root = root_snapshot(root)?;
        let account = self
            .identity_account(root.user_id)
            .await?
            .ok_or(SecurityError::CredentialRevoked)?;
        root.validate(&account, now(self.clock.as_ref())?)
            .map_err(|e| match e {
                crate::domain::identity::error::IdentityError::DependencyUnavailable => {
                    SecurityError::DependencyUnavailable
                }
                _ => SecurityError::CredentialRevoked,
            })?;
        if root.principal.issuer() != session.issuer
            || root.principal.subject() != session.subject
            || session.auth_epoch
                != Some(
                    i64::try_from(root.auth_epoch).map_err(|_| SecurityError::InvalidCredential)?,
                )
            || session.principal_kind != "user"
        {
            return Err(SecurityError::InvalidCredential);
        }
        let managed = if let Some(environment_id) = session.environment_id {
            let scopes = IdentityScopeSet::parse(
                session
                    .identity_scopes
                    .as_deref()
                    .ok_or(SecurityError::InvalidCredential)?,
            )?;
            let admission = self
                .current_identity_admission(&AdmissionQuery {
                    sso_session_id: root_id,
                    environment_id,
                    requested_scopes: scopes,
                })
                .await
                .map_err(|e| match e {
                    crate::domain::identity::error::IdentityError::DependencyUnavailable
                    | crate::domain::identity::error::IdentityError::DeadlineExceeded => e.into(),
                    _ => SecurityError::CredentialRevoked,
                })?;
            if session.config_version_id != Some(admission.config_version_id())
                || session.lifecycle_epoch.map(unsigned).transpose()?
                    != Some(admission.lifecycle_epoch())
                || session.access_generation.map(unsigned).transpose()?
                    != Some(admission.access_generation())
            {
                return Err(SecurityError::CredentialRevoked);
            }
            let rp = self
                .run(
                    identity_oidc_rp_session::Entity::find_by_id(
                        session
                            .oidc_rp_session_id
                            .ok_or(SecurityError::InvalidCredential)?,
                    )
                    .one(&self.transaction),
                )
                .await?
                .ok_or(SecurityError::CredentialRevoked)?;
            let client = self
                .run(oauth_client::Entity::find_by_id(rp.oauth_client_id).one(&self.transaction))
                .await?
                .ok_or(SecurityError::CredentialRevoked)?;
            if rp.revoked_at.is_some()
                || rp.sso_session_id != root_id
                || rp.environment_id != environment_id
                || rp.config_version_id != admission.config_version_id()
                || rp.subject != root.principal.subject()
                || rp.issuer != root.principal.issuer()
                || unsigned(rp.auth_time)? != admission.auth_time()
                || rp.identity_scopes != admission.scopes().as_protocol_string()
                || credential.oidc_client_id.as_deref() != Some(client.client_id.as_str())
            {
                return Err(SecurityError::CredentialRevoked);
            }
            Some(IdentityTokenContext {
                admission,
                client_id: client.client_id,
                protocol_sid: rp.protocol_sid,
                nonce: rp.nonce,
                root_expires_at: root.expires_at,
            })
        } else {
            if session.oidc_rp_session_id.is_some()
                || session.config_version_id.is_some()
                || credential.oidc_client_id.is_some()
            {
                return Err(SecurityError::InvalidCredential);
            }
            None
        };
        Ok(Some(IdentitySessionState {
            root_expires_at: root.expires_at,
            managed,
        }))
    }
}
