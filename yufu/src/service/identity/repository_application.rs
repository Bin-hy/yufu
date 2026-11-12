use super::repository::pg::{
    environment_kind, get, management_mode, one_row, signed, timestamp, unsigned,
};
use super::repository::*;
use crate::domain::identity::account::{ManagementMode, owner_is_qualified};
use crate::domain::identity::application::{
    AppEnvironmentKind, ApprovalSnapshot, EnvironmentConfig, LogoutCapability,
    requires_candidate_client, validate_review,
};
use crate::domain::identity::error::IdentityError;
use crate::service::security::repository::{PgSecurityTransaction, now};
use auth::security::identity::SecretToken;
use sha2::{Digest, Sha256};
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn write_identity_application(
        &mut self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        match command {
            IdentityWrite::UpdateApplicationMetadata {
                application_id,
                name,
                description,
                expected_revision,
            } => {
                one_row(self.identity_execute("UPDATE application SET name=$3,description=$4,identity_revision=identity_revision+1,update_time=$5 WHERE id=$1 AND identity_revision=$2 AND management_state='managed'",vec![application_id.into(),signed(expected_revision)?.into(),name.clone().into(),description.into(),timestamp(at)?.into()]).await?)?;
                self.identity_execute("UPDATE oauth_client SET name=$2,update_time=$3 WHERE environment_id IN (SELECT id FROM identity_app_environment WHERE application_id=$1)",vec![application_id.into(),name.into(),timestamp(at)?.into()]).await?;
                Ok(IdentityWriteResult::Revision(
                    expected_revision
                        .checked_add(1)
                        .ok_or(IdentityError::CapacityExceeded)?,
                ))
            }
            IdentityWrite::RegisterApplication(registration) => {
                let owner = self
                    .identity_account(registration.owner_user_id)
                    .await?
                    .ok_or(IdentityError::OwnerRequired)?;
                if !owner_is_qualified(&owner, registration.management_mode) {
                    return Err(IdentityError::OwnerRequired);
                }
                auth::security::policy::ResourceRef::new(
                    &registration.app_key,
                    auth::security::policy::ResourceTarget::Application,
                )?;
                if registration.name.trim().is_empty()
                    || registration.name.len() > 200
                    || registration.development.id == registration.production.id
                    || registration.development.client_id == registration.production.client_id
                {
                    return Err(IdentityError::InvalidInput);
                }
                let row = self.identity_one("INSERT INTO application (app_key,name,description,management_state,application_kind,management_mode,lifecycle,create_time,update_time) VALUES ($1,$2,$3,'managed',$4,$5,'active',$6,$6) RETURNING id", vec![registration.app_key.clone().into(), registration.name.clone().into(), registration.description.clone().into(), if registration.third_party { "third_party" } else { "internal" }.into(), mode_name(registration.management_mode).into(), timestamp(at)?.into()]).await?;
                let app_id: i32 = get(&row, "id")?;
                self.identity_execute("INSERT INTO identity_app_member (application_id,user_id,member_role,created_by,created_at) VALUES ($1,$2,'owner',$2,$3)",vec![app_id.into(),owner.user_id.into(),at.into()]).await?;
                for (kind, environment) in [
                    (AppEnvironmentKind::Development, &registration.development),
                    (AppEnvironmentKind::Production, &registration.production),
                ] {
                    let config = EnvironmentConfig::from_payload(
                        environment.config.to_payload()?,
                        kind,
                        kind == AppEnvironmentKind::Development,
                    )?;
                    let client = self
                        .insert_identity_client(
                            environment.id,
                            &environment.client_id,
                            environment.client_secret_hash.as_ref(),
                            &config,
                            owner.user_id,
                            &registration.name,
                            &registration.app_key,
                            kind == AppEnvironmentKind::Development,
                            at,
                        )
                        .await?;
                    self.identity_execute("INSERT INTO identity_app_environment (id,application_id,kind,oauth_client_id,active_version_id,created_at) VALUES ($1,$2,$3,$4,$5,$6)",vec![environment.id.into(),app_id.into(),kind_name(kind).into(),client.into(),if kind==AppEnvironmentKind::Development { Some(environment.initial_version_id) } else { None }.into(),at.into()]).await?;
                    self.insert_identity_version(
                        environment.initial_version_id,
                        environment.id,
                        1,
                        &config,
                        None,
                        owner.user_id,
                        at,
                    )
                    .await?;
                }
                Ok(IdentityWriteResult::Application {
                    application_id: app_id,
                    development_client_id: registration.development.client_id.clone(),
                    production_client_id: registration.production.client_id.clone(),
                })
            }
            IdentityWrite::SetMember {
                application_id,
                user_id,
                present,
                actor_id,
                expected_revision,
            } => {
                self.bump_application(application_id, expected_revision, "access_revision", at)
                    .await?;
                if present {
                    self.identity_execute("INSERT INTO identity_app_member (application_id,user_id,member_role,created_by,created_at) VALUES ($1,$2,'maintainer',$3,$4) ON CONFLICT (application_id,user_id) DO NOTHING",vec![application_id.into(),user_id.into(),actor_id.into(),at.into()]).await?;
                } else {
                    self.identity_execute("DELETE FROM identity_app_member WHERE application_id=$1 AND user_id=$2 AND member_role='maintainer'",vec![application_id.into(),user_id.into()]).await?;
                }
                self.obsolete_identity_approvals(application_id, at).await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::TransferOwner {
                application_id,
                owner_user_id,
                expected_revision,
            } => {
                let app = self.identity_one("SELECT management_mode FROM application WHERE id=$1 AND management_state='managed'",vec![application_id.into()]).await?;
                let account = self
                    .identity_account(owner_user_id)
                    .await?
                    .ok_or(IdentityError::OwnerRequired)?;
                if !owner_is_qualified(
                    &account,
                    management_mode(&get::<String>(&app, "management_mode")?)?,
                ) {
                    return Err(IdentityError::OwnerRequired);
                }
                self.bump_application(application_id, expected_revision, "owner_revision", at)
                    .await?;
                self.identity_execute("UPDATE identity_app_member SET member_role='maintainer' WHERE application_id=$1 AND member_role='owner'",vec![application_id.into()]).await?;
                self.identity_execute("INSERT INTO identity_app_member (application_id,user_id,member_role,created_by,created_at) VALUES ($1,$2,'owner',$2,$3) ON CONFLICT (application_id,user_id) DO UPDATE SET member_role='owner'",vec![application_id.into(),owner_user_id.into(),at.into()]).await?;
                self.obsolete_identity_approvals(application_id, at).await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CreateCandidateClient {
                environment_id,
                client_id,
                client_secret_hash,
                config,
                actor_id,
            } => {
                let env = self.identity_one("SELECT e.kind,a.app_key,a.name FROM identity_app_environment e JOIN application a ON a.id=e.application_id WHERE e.id=$1",vec![environment_id.into()]).await?;
                let config = EnvironmentConfig::from_payload(
                    config.to_payload()?,
                    environment_kind(&get::<String>(&env, "kind")?)?,
                    environment_kind(&get::<String>(&env, "kind")?)?
                        == AppEnvironmentKind::Development,
                )?;
                let client = self
                    .insert_identity_client(
                        environment_id,
                        &client_id,
                        client_secret_hash.as_ref(),
                        &config,
                        actor_id,
                        &get::<String>(&env, "name")?,
                        &get::<String>(&env, "app_key")?,
                        false,
                        at,
                    )
                    .await?;
                Ok(IdentityWriteResult::Client {
                    oauth_client_id: client,
                    client_id,
                })
            }
            IdentityWrite::InsertVersion(version) => {
                let env = self.identity_one("SELECT e.kind,e.active_version_id,e.application_id,c.public_client FROM identity_app_environment e JOIN oauth_client c ON c.id=e.oauth_client_id WHERE e.id=$1 AND e.revision=$2",vec![version.environment_id.into(),signed(version.expected_revision)?.into()]).await?;
                let kind = environment_kind(&get::<String>(&env, "kind")?)?;
                let config =
                    EnvironmentConfig::from_payload(version.config.to_payload()?, kind, false)?;
                let active: Option<Uuid> = get(&env, "active_version_id")?;
                if get::<bool>(&env, "public_client")? == config.permits_client_secret()
                    && version.candidate_client_id.is_none()
                {
                    return Err(IdentityError::InvalidInput);
                }
                if let Some(active) = active {
                    let row = self
                        .identity_one(
                            "SELECT payload FROM identity_app_config_version WHERE id=$1",
                            vec![active.into()],
                        )
                        .await?;
                    let current =
                        EnvironmentConfig::from_payload(get(&row, "payload")?, kind, true)?;
                    if requires_candidate_client(current.client_kind, config.client_kind)
                        && version.candidate_client_id.is_none()
                    {
                        return Err(IdentityError::InvalidInput);
                    }
                }
                if let Some(candidate) = version.candidate_client_id {
                    let row = self.identity_one("SELECT public_client FROM oauth_client WHERE id=$1 AND environment_id=$2 AND management_state='managed' AND id<>(SELECT oauth_client_id FROM identity_app_environment WHERE id=$2)",vec![candidate.into(),version.environment_id.into()]).await?;
                    if get::<bool>(&row, "public_client")? == config.permits_client_secret() {
                        return Err(IdentityError::InvalidInput);
                    }
                }
                let row = self.identity_one("SELECT COALESCE(MAX(version),0)+1 AS next_version FROM identity_app_config_version WHERE environment_id=$1",vec![version.environment_id.into()]).await?;
                self.insert_identity_version(
                    version.id,
                    version.environment_id,
                    get(&row, "next_version")?,
                    &config,
                    version.candidate_client_id,
                    version.actor_id,
                    at,
                )
                .await?;
                one_row(self.identity_execute("UPDATE identity_app_environment SET revision=revision+1 WHERE id=$1 AND revision=$2",vec![version.environment_id.into(),signed(version.expected_revision)?.into()]).await?)?;
                self.obsolete_identity_approvals(get(&env, "application_id")?, at)
                    .await?;
                Ok(IdentityWriteResult::Resource(version.id))
            }
            IdentityWrite::SubmitApproval(approval) => {
                let mut current = self
                    .identity_approval_snapshot(
                        approval.snapshot.environment_id,
                        approval.snapshot.config_version_id,
                        approval.snapshot.logout_limitation_accepted,
                    )
                    .await?;
                if current != approval.snapshot {
                    return Err(IdentityError::ApprovalObsolete);
                }
                if current.logout_limitation_accepted {
                    return Err(IdentityError::InvalidInput);
                }
                self.identity_version_config(current.config_version_id, true)
                    .await?;
                // The frozen request contains the revision after submission;
                // otherwise its own revision bump would obsolete it immediately.
                one_row(self.identity_execute("UPDATE identity_app_environment SET revision=revision+1 WHERE id=$1 AND kind='production' AND revision=$2",vec![current.environment_id.into(),signed(current.environment_revision)?.into()]).await?)?;
                current.environment_revision = current
                    .environment_revision
                    .checked_add(1)
                    .ok_or(IdentityError::CapacityExceeded)?;
                let payload =
                    serde_json::to_value(&current).map_err(|_| IdentityError::InvalidInput)?;
                self.identity_execute("INSERT INTO identity_app_approval (id,environment_id,version_id,snapshot,snapshot_hash,requested_by,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7)",vec![approval.id.into(),current.environment_id.into(),current.config_version_id.into(),payload.into(),current.digest()?.into(),approval.requested_by.into(),at.into()]).await?;
                Ok(IdentityWriteResult::Resource(approval.id))
            }
            IdentityWrite::ReviewApproval {
                approval_id,
                reviewer_id,
                approve,
                reason,
                accept_logout_limitation,
            } => {
                let reviewer = self
                    .identity_account(reviewer_id)
                    .await?
                    .ok_or(IdentityError::Forbidden)?;
                if !reviewer.enabled || !reviewer.administrator {
                    return Err(IdentityError::Forbidden);
                }
                let row = self.identity_one("SELECT requested_by,snapshot,snapshot_hash FROM identity_app_approval WHERE id=$1 AND status='pending'",vec![approval_id.into()]).await?;
                let saved: ApprovalSnapshot = serde_json::from_value(get(&row, "snapshot")?)
                    .map_err(|_| IdentityError::DependencyUnavailable)?;
                let current = self
                    .identity_approval_snapshot(
                        saved.environment_id,
                        saved.config_version_id,
                        saved.logout_limitation_accepted,
                    )
                    .await?;
                let config = self
                    .identity_version_config(saved.config_version_id, true)
                    .await?;
                if saved.digest()? != get::<String>(&row, "snapshot_hash")? {
                    return Err(IdentityError::ApprovalObsolete);
                }
                let mut review_saved = saved.clone();
                let mut review_current = current.clone();
                review_saved.logout_limitation_accepted = approve && accept_logout_limitation;
                review_current.logout_limitation_accepted = approve && accept_logout_limitation;
                if saved != current {
                    return Err(IdentityError::ApprovalObsolete);
                }
                validate_review(
                    get(&row, "requested_by")?,
                    reviewer_id,
                    &review_saved,
                    &review_current,
                    if approve {
                        config.logout_capability
                    } else {
                        LogoutCapability::Online
                    },
                )?;
                one_row(self.identity_execute("UPDATE identity_app_approval SET status=$2,reviewed_by=$3,reason=$4,reviewed_at=$5,logout_limitation_accepted=$6 WHERE id=$1 AND status='pending'",vec![approval_id.into(),if approve { "approved" } else { "rejected" }.into(),reviewer_id.into(),reason.into(),at.into(),(approve && accept_logout_limitation).into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::RecordAdminActivation { snapshot, actor_id } => {
                let actor = self
                    .identity_account(actor_id)
                    .await?
                    .ok_or(IdentityError::Forbidden)?;
                if !actor.enabled || !actor.administrator {
                    return Err(IdentityError::Forbidden);
                }
                let current = self
                    .identity_approval_snapshot(
                        snapshot.environment_id,
                        snapshot.config_version_id,
                        snapshot.logout_limitation_accepted,
                    )
                    .await?;
                if current != snapshot {
                    return Err(IdentityError::ApprovalObsolete);
                }
                let config = self
                    .identity_version_config(snapshot.config_version_id, true)
                    .await?;
                if config.logout_capability == LogoutCapability::LocalOnly
                    && !snapshot.logout_limitation_accepted
                {
                    return Err(IdentityError::InvalidInput);
                }
                self.identity_execute("INSERT INTO identity_admin_activation (id,environment_id,version_id,actor_id,snapshot,logout_limitation_accepted,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7)",vec![Uuid::new_v4().into(),snapshot.environment_id.into(),snapshot.config_version_id.into(),actor_id.into(),serde_json::to_value(&snapshot).map_err(|_|IdentityError::InvalidInput)?.into(),snapshot.logout_limitation_accepted.into(),at.into()]).await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::ActivateVersion {
                environment_id,
                version_id,
                expected_revision,
            } => {
                let env = self.identity_one("SELECT e.kind,e.oauth_client_id,e.application_id,a.app_key,a.management_mode,a.lifecycle FROM identity_app_environment e JOIN application a ON a.id=e.application_id WHERE e.id=$1 AND e.revision=$2 AND a.management_state='managed'",vec![environment_id.into(),signed(expected_revision)?.into()]).await?;
                let kind = environment_kind(&get::<String>(&env, "kind")?)?;
                let config = self.identity_version_config(version_id, true).await?;
                if kind == AppEnvironmentKind::Production {
                    let mode = management_mode(&get::<String>(&env, "management_mode")?)?;
                    let row = if mode == ManagementMode::AdminConfigured {
                        self.identity_one("SELECT snapshot FROM identity_admin_activation WHERE environment_id=$1 AND version_id=$2 ORDER BY created_at DESC LIMIT 1",vec![environment_id.into(),version_id.into()]).await?
                    } else {
                        self.identity_one("SELECT snapshot,logout_limitation_accepted FROM identity_app_approval WHERE environment_id=$1 AND version_id=$2 AND status='approved' ORDER BY created_at DESC LIMIT 1",vec![environment_id.into(),version_id.into()]).await?
                    };
                    let saved: ApprovalSnapshot = serde_json::from_value(get(&row, "snapshot")?)
                        .map_err(|_| IdentityError::DependencyUnavailable)?;
                    if saved
                        != self
                            .identity_approval_snapshot(
                                environment_id,
                                version_id,
                                saved.logout_limitation_accepted,
                            )
                            .await?
                    {
                        return Err(IdentityError::ApprovalObsolete);
                    }
                    let accepted = if mode == ManagementMode::AdminConfigured {
                        saved.logout_limitation_accepted
                    } else {
                        get::<bool>(&row, "logout_limitation_accepted")?
                    };
                    if config.logout_capability == LogoutCapability::LocalOnly && !accepted {
                        return Err(IdentityError::InvalidInput);
                    }
                }
                let version = self.identity_one("SELECT candidate_oauth_client_id FROM identity_app_config_version WHERE id=$1 AND environment_id=$2",vec![version_id.into(),environment_id.into()]).await?;
                let old_client: i32 = get(&env, "oauth_client_id")?;
                let client: i32 = get::<Option<i32>>(&version, "candidate_oauth_client_id")?
                    .unwrap_or(old_client);
                let row = self.identity_one("SELECT client_id,public_client FROM oauth_client WHERE id=$1 AND environment_id=$2",vec![client.into(),environment_id.into()]).await?;
                if get::<bool>(&row, "public_client")? == config.permits_client_secret() {
                    return Err(IdentityError::InvalidInput);
                }
                self.identity_execute("UPDATE security_session_client SET enabled=FALSE WHERE client_id IN (SELECT client_id FROM oauth_client WHERE environment_id=$1)",vec![environment_id.into()]).await?;
                self.mirror_identity_client(
                    client,
                    &config,
                    &get::<String>(&env, "app_key")?,
                    get::<String>(&env, "lifecycle")? == "active",
                    at,
                )
                .await?;
                one_row(self.identity_execute("UPDATE identity_app_environment SET active_version_id=$2,oauth_client_id=$3,lifecycle_epoch=lifecycle_epoch+1,revision=revision+1 WHERE id=$1 AND revision=$4",vec![environment_id.into(),version_id.into(),client.into(),signed(expected_revision)?.into()]).await?)?;
                self.revoke_identity_scope(
                    &crate::domain::identity::session::IdentityRevocationScope::Environment {
                        environment_id,
                    },
                    at,
                )
                .await?;
                Ok(IdentityWriteResult::Revision(
                    expected_revision
                        .checked_add(1)
                        .ok_or(IdentityError::CapacityExceeded)?,
                ))
            }
            IdentityWrite::SetApplicationLifecycle {
                application_id,
                active,
                expected_revision,
            } => {
                one_row(self.identity_execute("UPDATE application SET lifecycle=$2,identity_revision=identity_revision+1,security_revision=security_revision+1,update_time=$4 WHERE id=$1 AND identity_revision=$3 AND management_state='managed'",vec![application_id.into(),if active { "active" } else { "suspended" }.into(),signed(expected_revision)?.into(),timestamp(at)?.into()]).await?)?;
                self.identity_execute("UPDATE identity_app_environment SET lifecycle_epoch=lifecycle_epoch+1,revision=revision+1 WHERE application_id=$1",vec![application_id.into()]).await?;
                self.identity_execute("UPDATE security_session_client SET enabled=$2 WHERE client_id IN (SELECT c.client_id FROM identity_app_environment e JOIN oauth_client c ON c.id=e.oauth_client_id WHERE e.application_id=$1 AND e.active_version_id IS NOT NULL)",vec![application_id.into(),active.into()]).await?;
                self.obsolete_identity_approvals(application_id, at).await?;
                let environments = self.identity_rows("SELECT id FROM identity_app_environment WHERE application_id=$1 ORDER BY id",vec![application_id.into()]).await?;
                for env in environments {
                    self.revoke_identity_scope(
                        &crate::domain::identity::session::IdentityRevocationScope::Environment {
                            environment_id: get(&env, "id")?,
                        },
                        at,
                    )
                    .await?;
                }
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::ResetEnvironmentSecret {
                environment_id,
                expected_revision,
                secret_hash,
            } => {
                let env=self.identity_one("SELECT e.application_id,e.oauth_client_id FROM identity_app_environment e JOIN oauth_client c ON c.id=e.oauth_client_id WHERE e.id=$1 AND e.revision=$2 AND c.management_state='managed' AND NOT c.public_client",vec![environment_id.into(),signed(expected_revision)?.into()]).await?;
                if secret_hash.expose_secret().is_empty() {
                    return Err(IdentityError::InvalidInput);
                }
                one_row(
                    self.identity_execute(
                        "UPDATE oauth_client SET client_secret_hash=$2,secret_revision=secret_revision+1,update_time=$3 WHERE id=$1",
                        vec![
                            get::<i32>(&env, "oauth_client_id")?.into(),
                            secret_hash.expose_secret().into(),
                            timestamp(at)?.into(),
                        ],
                    )
                    .await?,
                )?;
                one_row(self.identity_execute("UPDATE identity_app_environment SET revision=revision+1,lifecycle_epoch=lifecycle_epoch+1 WHERE id=$1 AND revision=$2",vec![environment_id.into(),signed(expected_revision)?.into()]).await?)?;
                self.obsolete_identity_approvals(get(&env, "application_id")?, at)
                    .await?;
                self.revoke_identity_scope(
                    &crate::domain::identity::session::IdentityRevocationScope::Environment {
                        environment_id,
                    },
                    at,
                )
                .await?;
                Ok(IdentityWriteResult::Revision(
                    expected_revision
                        .checked_add(1)
                        .ok_or(IdentityError::CapacityExceeded)?,
                ))
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }

    pub(crate) async fn identity_approval_snapshot(
        &self,
        environment: Uuid,
        version: Uuid,
        accept_local: bool,
    ) -> Result<ApprovalSnapshot, IdentityError> {
        let row = self.identity_one("SELECT e.revision,m.user_id,a.owner_revision,a.access_revision,a.security_revision,v.payload_hash FROM identity_app_environment e JOIN application a ON a.id=e.application_id JOIN identity_app_member m ON m.application_id=a.id AND m.member_role='owner' JOIN identity_app_config_version v ON v.environment_id=e.id AND v.id=$2 WHERE e.id=$1",vec![environment.into(),version.into()]).await?;
        Ok(ApprovalSnapshot {
            environment_id: environment,
            config_version_id: version,
            config_hash: get(&row, "payload_hash")?,
            environment_revision: unsigned(get(&row, "revision")?)?,
            owner_user_id: get(&row, "user_id")?,
            owner_revision: unsigned(get(&row, "owner_revision")?)?,
            access_revision: unsigned(get(&row, "access_revision")?)?,
            security_revision: unsigned(get(&row, "security_revision")?)?,
            logout_limitation_accepted: accept_local,
        })
    }
    pub(crate) async fn identity_version_config(
        &self,
        version: Uuid,
        activation: bool,
    ) -> Result<EnvironmentConfig, IdentityError> {
        let row = self.identity_one("SELECT v.payload,e.kind FROM identity_app_config_version v JOIN identity_app_environment e ON e.id=v.environment_id WHERE v.id=$1",vec![version.into()]).await?;
        EnvironmentConfig::from_payload(
            get(&row, "payload")?,
            environment_kind(&get::<String>(&row, "kind")?)?,
            activation,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn insert_identity_version(
        &self,
        id: Uuid,
        environment: Uuid,
        number: i64,
        config: &EnvironmentConfig,
        candidate: Option<i32>,
        actor: i32,
        at: i64,
    ) -> Result<(), IdentityError> {
        let payload = config.to_payload()?;
        let hash = hex::encode(Sha256::digest(
            serde_json::to_vec(&payload).map_err(|_| IdentityError::InvalidInput)?,
        ));
        self.identity_execute("INSERT INTO identity_app_config_version (id,environment_id,version,payload,payload_hash,candidate_oauth_client_id,created_by,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",vec![id.into(),environment.into(),number.into(),payload.into(),hash.into(),candidate.into(),actor.into(),at.into()]).await?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn insert_identity_client(
        &self,
        environment: Uuid,
        client_id: &str,
        secret: Option<&SecretToken>,
        config: &EnvironmentConfig,
        owner: i32,
        name: &str,
        app_key: &str,
        enabled: bool,
        at: i64,
    ) -> Result<i32, IdentityError> {
        if client_id.is_empty()
            || client_id.len() > 128
            || config.permits_client_secret() != secret.is_some()
            || secret.is_some_and(|s| s.expose_secret().is_empty())
        {
            return Err(IdentityError::InvalidInput);
        }
        let row = self.identity_one("INSERT INTO oauth_client (client_id,user_id,name,redirect_uris,allowed_scopes,client_secret_hash,public_client,grant_types,management_state,environment_id,create_time,update_time) VALUES ($1,$2,$3,$4,$5,$6,$7,'authorization_code,refresh_token','managed',$8,$9,$9) RETURNING id",vec![client_id.into(),owner.into(),name.into(),redirect_mirror(config).into(),scope_mirror(config).into(),secret.map(|s|s.expose_secret()).unwrap_or("").into(),(!config.permits_client_secret()).into(),environment.into(),timestamp(at)?.into()]).await?;
        let id = get(&row, "id")?;
        self.identity_execute("INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) SELECT issuer,$1,$2,$3,$4 FROM identity_provider WHERE id=1",vec![client_id.into(),config.access_audience.as_str().into(),app_key.into(),enabled.into()]).await.and_then(one_row)?;
        Ok(id)
    }
    async fn mirror_identity_client(
        &self,
        client: i32,
        config: &EnvironmentConfig,
        app_key: &str,
        enabled: bool,
        at: i64,
    ) -> Result<(), IdentityError> {
        one_row(self.identity_execute("UPDATE oauth_client SET redirect_uris=$2,allowed_scopes=$3,grant_types='authorization_code,refresh_token',update_time=$4 WHERE id=$1",vec![client.into(),redirect_mirror(config).into(),scope_mirror(config).into(),timestamp(at)?.into()]).await?)?;
        one_row(self.identity_execute("UPDATE security_session_client SET audience=$2,application=$3,enabled=$4 WHERE client_id=(SELECT client_id FROM oauth_client WHERE id=$1)",vec![client.into(),config.access_audience.as_str().into(),app_key.into(),enabled.into()]).await?)
    }
    async fn bump_application(
        &self,
        app: i32,
        expected: u64,
        column: &str,
        at: i64,
    ) -> Result<(), IdentityError> {
        if !["access_revision", "owner_revision"].contains(&column) {
            return Err(IdentityError::InvalidInput);
        }
        one_row(self.identity_execute(&format!("UPDATE application SET identity_revision=identity_revision+1,{column}={column}+1,update_time=$3 WHERE id=$1 AND identity_revision=$2 AND management_state='managed'"),vec![app.into(),signed(expected)?.into(),timestamp(at)?.into()]).await?)
    }
    pub(crate) async fn obsolete_identity_approvals(
        &self,
        app: i32,
        _at: i64,
    ) -> Result<(), IdentityError> {
        self.identity_execute("UPDATE identity_app_approval SET status='obsolete' WHERE status='pending' AND environment_id IN (SELECT id FROM identity_app_environment WHERE application_id=$1)",vec![app.into()]).await?;
        Ok(())
    }
}
fn kind_name(kind: AppEnvironmentKind) -> &'static str {
    match kind {
        AppEnvironmentKind::Development => "development",
        AppEnvironmentKind::Production => "production",
    }
}
fn mode_name(mode: ManagementMode) -> &'static str {
    match mode {
        ManagementMode::DeveloperManaged => "developer_managed",
        ManagementMode::AdminConfigured => "admin_configured",
    }
}
fn redirect_mirror(config: &EnvironmentConfig) -> String {
    config
        .redirect_uris
        .iter()
        .map(|u| u.as_str())
        .collect::<Vec<_>>()
        .join(",")
}
fn scope_mirror(config: &EnvironmentConfig) -> String {
    config
        .identity_scopes
        .as_protocol_string()
        .replace(' ', ",")
}
