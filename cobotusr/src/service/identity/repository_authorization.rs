use super::repository::pg::{environment_kind, get, management_mode, signed, timestamp};
use super::repository::*;
use crate::domain::identity::{
    account::owner_is_qualified,
    application::{EnvironmentConfig, IdentityScopeSet},
    error::IdentityError,
};
use crate::service::security::repository::PgSecurityTransaction;
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn identity_code_token_context(
        &self,
        code: &crate::entity::oauth_authorization_code::Model,
    ) -> Result<crate::domain::identity::session::IdentityTokenContext, IdentityError> {
        use super::repository::pg::{root_snapshot, unsigned};
        use crate::entity::{identity_oidc_rp_session, identity_sso_session};
        use sea_orm::EntityTrait;
        let rp_id = code.oidc_rp_session_id.ok_or(IdentityError::AccessDenied)?;
        let rp = self
            .run(identity_oidc_rp_session::Entity::find_by_id(rp_id).one(&self.transaction))
            .await?
            .ok_or(IdentityError::AccessDenied)?;
        let root = self
            .run(identity_sso_session::Entity::find_by_id(rp.sso_session_id).one(&self.transaction))
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        let root = root_snapshot(root)?;
        let scope =
            IdentityScopeSet::parse(code.scope.as_deref().ok_or(IdentityError::AccessDenied)?)?;
        let admission = self
            .current_identity_admission(&AdmissionQuery {
                sso_session_id: root.id,
                environment_id: rp.environment_id,
                requested_scopes: scope,
            })
            .await?;
        let client = self
            .identity_one(
                "SELECT client_id FROM oauth_client WHERE id=$1",
                vec![rp.oauth_client_id.into()],
            )
            .await?;
        if rp.revoked_at.is_some()
            || code.used
            || code.code_digest.is_none()
            || code.sso_session_id != Some(root.id)
            || code.environment_id != Some(rp.environment_id)
            || code.config_version_id != Some(admission.config_version_id())
            || rp.config_version_id != admission.config_version_id()
            || code.auth_epoch != Some(signed(admission.auth_epoch())?)
            || code.lifecycle_epoch != Some(signed(admission.lifecycle_epoch())?)
            || code.access_generation != Some(signed(admission.access_generation())?)
            || code.auth_time != Some(signed(admission.auth_time())?)
            || unsigned(rp.auth_time)? != admission.auth_time()
            || rp.subject != root.principal.subject()
            || rp.issuer != root.principal.issuer()
            || rp.identity_scopes != admission.scopes().as_protocol_string()
            || rp.nonce != code.nonce
            || get::<String>(&client, "client_id")? != code.client_id
        {
            return Err(IdentityError::AccessDenied);
        }
        Ok(crate::domain::identity::session::IdentityTokenContext {
            admission,
            client_id: code.client_id.clone(),
            protocol_sid: rp.protocol_sid,
            nonce: rp.nonce,
            root_expires_at: root.expires_at,
        })
    }
    pub(crate) async fn identity_authorization_target(
        &self,
        client_id: &str,
        user_id: Option<i32>,
    ) -> Result<AuthorizationTarget, IdentityError> {
        let row = self.identity_rows("SELECT a.id AS application_id,a.name,a.management_mode,a.lifecycle,e.id AS environment_id,e.kind,e.active_version_id,v.payload,b.enabled,b.application,b.audience,a.app_key FROM oauth_client c JOIN identity_app_environment e ON e.oauth_client_id=c.id AND c.environment_id=e.id JOIN application a ON a.id=e.application_id JOIN identity_app_config_version v ON v.id=e.active_version_id AND v.environment_id=e.id LEFT JOIN security_session_client b ON b.client_id=c.client_id AND b.issuer=(SELECT issuer FROM identity_provider WHERE id=1) WHERE c.client_id=$1 AND c.management_state='managed' AND a.management_state='managed'",vec![client_id.into()]).await?;
        let row = row.first().ok_or(IdentityError::AccessDenied)?;
        let application_id = get::<i32>(row, "application_id")?;
        let environment_id = get::<Uuid>(row, "environment_id")?;
        let config = EnvironmentConfig::from_payload(
            get(row, "payload")?,
            environment_kind(&get::<String>(row, "kind")?)?,
            true,
        )?;
        let mut keys = vec![
            IdentityLockKey::Application(application_id),
            IdentityLockKey::Environment(environment_id),
        ];
        let owners = self.identity_rows("SELECT user_id FROM identity_app_member WHERE application_id=$1 AND member_role='owner'",vec![application_id.into()]).await?;
        let mut qualified = false;
        for owner in owners {
            let owner_id = get::<i32>(&owner, "user_id")?;
            keys.push(IdentityLockKey::User(owner_id));
            if let Some(account) = self.identity_account(owner_id).await? {
                qualified = owner_is_qualified(
                    &account,
                    management_mode(&get::<String>(row, "management_mode")?)?,
                );
            }
        }
        for group in self.identity_rows("SELECT group_id FROM identity_app_assignment WHERE application_id=$1 AND target_kind='group' ORDER BY group_id",vec![application_id.into()]).await? {
            keys.push(IdentityLockKey::Group(get(&group,"group_id")?));
        }
        if let Some(id) = user_id {
            keys.push(IdentityLockKey::User(id));
        }
        let enabled = qualified
            && get::<String>(row, "lifecycle")? == "active"
            && get::<Option<bool>>(row, "enabled")? == Some(true)
            && get::<Option<String>>(row, "application")? == Some(get(row, "app_key")?)
            && get::<Option<String>>(row, "audience")?
                == Some(config.access_audience.as_str().into());
        Ok(AuthorizationTarget {
            application_id,
            application_name: get(row, "name")?,
            environment_id,
            config_version_id: get(row, "active_version_id")?,
            config,
            enabled,
            locks: IdentityLockScope::new(keys)?,
        })
    }

    pub(crate) async fn identity_current_consent(
        &self,
        user_id: i32,
        environment_id: Uuid,
        version_id: Uuid,
    ) -> Result<Option<IdentityScopeSet>, IdentityError> {
        self.identity_rows("SELECT scope FROM oauth_consent WHERE user_id=$1 AND environment_id=$2 AND config_version_id=$3",vec![user_id.into(),environment_id.into(),version_id.into()]).await?.first().map(|row| IdentityScopeSet::parse(&get::<String>(row,"scope")?).map_err(|_|IdentityError::DependencyUnavailable)).transpose()
    }

    pub(crate) async fn save_identity_consent(
        &self,
        user_id: i32,
        environment_id: Uuid,
        version_id: Uuid,
        scopes: IdentityScopeSet,
        at: i64,
    ) -> Result<(), IdentityError> {
        let config = self.identity_version_config(version_id, true).await?;
        if !scopes.is_subset_of(&config.identity_scopes) {
            return Err(IdentityError::AccessDenied);
        }
        let current = self
            .identity_current_consent(user_id, environment_id, version_id)
            .await?;
        let scope = match current {
            Some(current) => IdentityScopeSet::parse(&format!(
                "{} {}",
                current.as_protocol_string(),
                scopes.as_protocol_string()
            ))?,
            None => scopes,
        };
        if !scope.is_subset_of(&config.identity_scopes) {
            return Err(IdentityError::AccessDenied);
        }
        super::repository::pg::one_row(self.identity_execute("INSERT INTO oauth_consent (user_id,client_id,scope,create_time,update_time,environment_id,config_version_id) SELECT $1,c.client_id,$2,$3,$3,e.id,$5 FROM identity_app_environment e JOIN oauth_client c ON c.id=e.oauth_client_id WHERE e.id=$4 AND e.active_version_id=$5 ON CONFLICT (user_id,environment_id,config_version_id) WHERE environment_id IS NOT NULL DO UPDATE SET scope=EXCLUDED.scope,update_time=EXCLUDED.update_time",vec![user_id.into(),scope.as_protocol_string().into(),timestamp(at)?.into(),environment_id.into(),version_id.into()]).await?)
    }

    pub(crate) async fn ensure_identity_generation(
        &self,
        environment_id: Uuid,
        user_id: i32,
        at: u64,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let account = self
            .identity_account(user_id)
            .await?
            .ok_or(IdentityError::Unauthenticated)?;
        let env = self.identity_one("SELECT e.kind,e.application_id,a.management_mode FROM identity_app_environment e JOIN application a ON a.id=e.application_id WHERE e.id=$1",vec![environment_id.into()]).await?;
        let sources = self
            .identity_sources(
                get(&env, "application_id")?,
                environment_id,
                &account,
                management_mode(&get::<String>(&env, "management_mode")?)?,
            )
            .await?;
        let eligible =
            account.enabled && sources.eligible(environment_kind(&get::<String>(&env, "kind")?)?);
        if !eligible {
            return Err(IdentityError::AccessDenied);
        }
        self.identity_execute("INSERT INTO identity_access_generation (environment_id,user_id,generation,eligible,changed_at) VALUES ($1,$2,0,$3,$4) ON CONFLICT (environment_id,user_id) DO NOTHING",vec![environment_id.into(),user_id.into(),eligible.into(),signed(at)?.into()]).await?;
        Ok(IdentityWriteResult::Changed)
    }
}
