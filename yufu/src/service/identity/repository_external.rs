use super::repository::pg::{get, one_row, signed, timestamp, unsigned};
use super::repository::{IdentityWrite, IdentityWriteResult};
use crate::domain::identity::{
    error::IdentityError,
    external::{
        ExternalFlowSnapshot, ExternalIdentityRecord, ExternalProviderAdminView,
        ExternalProviderKey, ExternalPurpose, ProviderStat, UpdateExternalProvider,
    },
};
use crate::service::security::repository::{PgSecurityTransaction, now};
use std::collections::BTreeMap;
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn identity_external_flow_by_digest(
        &self,
        digest: &crate::domain::identity::account::SecretDigest,
    ) -> Result<Option<ExternalFlowSnapshot>, IdentityError> {
        let rows = self
            .identity_rows(
                "SELECT * FROM identity_external_flow WHERE state_digest=$1",
                vec![digest.as_bytes().to_vec().into()],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(|row| {
                Ok(ExternalFlowSnapshot {
                    id: get(&row, "id")?,
                    provider_key: get::<String>(&row, "provider_key")?.parse()?,
                    purpose: match get::<String>(&row, "purpose")?.as_str() {
                        "login" => ExternalPurpose::Login,
                        "bind" => ExternalPurpose::Bind,
                        _ => return Err(IdentityError::DependencyUnavailable),
                    },
                    browser_binding_digest:
                        crate::domain::identity::account::SecretDigest::from_bytes(
                            get::<Vec<u8>>(&row, "browser_binding_digest")?
                                .try_into()
                                .map_err(|_| IdentityError::DependencyUnavailable)?,
                        ),
                    user_id: get(&row, "user_id")?,
                    oidc_flow_id: get(&row, "oidc_flow_id")?,
                    nonce: get(&row, "nonce")?,
                    secret_ciphertext: get(&row, "secret_ciphertext")?,
                    expires_at: unsigned(get(&row, "expires_at")?)?,
                    consumed_at: get::<Option<i64>>(&row, "consumed_at")?
                        .map(unsigned)
                        .transpose()?,
                })
            })
            .transpose()
    }

    pub(crate) async fn identity_external_identity_by_subject(
        &self,
        provider_key: ExternalProviderKey,
        subject: &str,
    ) -> Result<Option<ExternalIdentityRecord>, IdentityError> {
        let rows = self
            .identity_rows(
                "SELECT * FROM identity_external_identity WHERE provider_key=$1 AND provider_subject=$2",
                vec![provider_key.as_str().into(), subject.into()],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(external_identity_row)
            .transpose()
    }

    pub(crate) async fn identity_external_identity_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ExternalIdentityRecord>, IdentityError> {
        let rows = self
            .identity_rows(
                "SELECT * FROM identity_external_identity WHERE id=$1",
                vec![id.into()],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(external_identity_row)
            .transpose()
    }

    pub(crate) async fn identity_external_identities_for_user(
        &self,
        user_id: i32,
    ) -> Result<Vec<ExternalIdentityRecord>, IdentityError> {
        self.identity_rows(
            "SELECT * FROM identity_external_identity WHERE user_id=$1 ORDER BY created_at,id",
            vec![user_id.into()],
        )
        .await?
        .into_iter()
        .map(external_identity_row)
        .collect()
    }

    /// Password plus every bound external identity. Used to refuse unbinding the
    /// final way a user can authenticate.
    pub(crate) async fn identity_external_login_credential_count(
        &self,
        user_id: i32,
    ) -> Result<u32, IdentityError> {
        let row = self
            .identity_one(
                "SELECT ((u.password IS NOT NULL)::int + (SELECT COUNT(*)::int FROM identity_external_identity e WHERE e.user_id=u.id))::bigint AS value FROM \"user\" u WHERE u.id=$1",
                vec![user_id.into()],
            )
            .await?;
        u32::try_from(get::<i64>(&row, "value")?).map_err(|_| IdentityError::DependencyUnavailable)
    }

    pub(crate) async fn identity_external_provider_admin_rows(
        &self,
    ) -> Result<Vec<ExternalProviderAdminView>, IdentityError> {
        self.identity_rows(
            "SELECT provider_key,family,enabled,client_id,secret_ciphertext IS NOT NULL AS secret_configured,revision,updated_at,updated_by FROM identity_external_provider ORDER BY provider_key",
            Vec::new(),
        )
        .await?
        .into_iter()
        .map(provider_admin_row)
        .collect()
    }

    pub(crate) async fn identity_update_external_provider(
        &mut self,
        provider_key: ExternalProviderKey,
        expected_revision: u64,
        update: &UpdateExternalProvider,
        secret_ciphertext: Option<String>,
        actor_id: i32,
    ) -> Result<ExternalProviderAdminView, IdentityError> {
        update.validate(provider_key)?;
        let at = signed(now(self.clock.as_ref())?)?;
        one_row(
            self.identity_execute(
                "UPDATE identity_external_provider SET enabled=$2,client_id=$3,secret_ciphertext=COALESCE($4,secret_ciphertext),revision=revision+1,updated_at=$5,updated_by=$6 WHERE provider_key=$1 AND revision=$7",
                vec![
                    provider_key.as_str().into(),
                    update.enabled.into(),
                    update.client_id.clone().into(),
                    secret_ciphertext.into(),
                    at.into(),
                    actor_id.into(),
                    signed(expected_revision)?.into(),
                ],
            )
            .await?,
        )?;
        self.identity_external_provider_admin_row(provider_key)
            .await
    }

    pub(crate) async fn identity_rotate_external_provider_secret(
        &mut self,
        provider_key: ExternalProviderKey,
        expected_revision: u64,
        secret_ciphertext: String,
        secret_key_version: u32,
        actor_id: i32,
    ) -> Result<ExternalProviderAdminView, IdentityError> {
        if secret_ciphertext.is_empty() || secret_key_version == 0 {
            return Err(IdentityError::InvalidInput);
        }
        let at = signed(now(self.clock.as_ref())?)?;
        one_row(
            self.identity_execute(
                "UPDATE identity_external_provider SET secret_ciphertext=$2,secret_key_version=$3,revision=revision+1,updated_at=$4,updated_by=$5 WHERE provider_key=$1 AND revision=$6",
                vec![
                    provider_key.as_str().into(),
                    secret_ciphertext.into(),
                    i32::try_from(secret_key_version)
                        .map_err(|_| IdentityError::InvalidInput)?
                        .into(),
                    at.into(),
                    actor_id.into(),
                    signed(expected_revision)?.into(),
                ],
            )
            .await?,
        )?;
        self.identity_external_provider_admin_row(provider_key)
            .await
    }

    pub(crate) async fn identity_external_login_stats(
        &self,
        from_millis: u64,
        to_millis: u64,
    ) -> Result<Vec<ProviderStat>, IdentityError> {
        if from_millis >= to_millis {
            return Err(IdentityError::InvalidInput);
        }
        let rows = self
            .identity_rows(
                "SELECT COALESCE(change_summary->>'provider','unknown') AS provider,result,COALESCE(reason_code,'none') AS reason,COUNT(*)::bigint AS value FROM identity_audit_event WHERE action='external.login' AND happened_at>=$1 AND happened_at<$2 GROUP BY 1,2,3 ORDER BY 1,2,3",
                vec![signed(from_millis)?.into(), signed(to_millis)?.into()],
            )
            .await?;
        let mut stats: BTreeMap<String, ProviderStat> = BTreeMap::new();
        for row in rows {
            let provider = get::<String>(&row, "provider")?;
            let result = get::<String>(&row, "result")?;
            let reason = get::<String>(&row, "reason")?;
            let count = u64::try_from(get::<i64>(&row, "value")?)
                .map_err(|_| IdentityError::DependencyUnavailable)?;
            let stat = stats
                .entry(provider.clone())
                .or_insert_with(|| ProviderStat {
                    provider_key: provider,
                    success: 0,
                    denied: 0,
                    failure: 0,
                    reasons: Vec::new(),
                });
            match result.as_str() {
                "success" => stat.success += count,
                "denied" => stat.denied += count,
                _ => stat.failure += count,
            }
            if result != "success" && reason != "none" {
                stat.reasons.push((reason, count));
            }
        }
        Ok(stats.into_values().collect())
    }

    async fn identity_external_provider_admin_row(
        &self,
        provider_key: ExternalProviderKey,
    ) -> Result<ExternalProviderAdminView, IdentityError> {
        let rows = self
            .identity_rows(
                "SELECT provider_key,family,enabled,client_id,secret_ciphertext IS NOT NULL AS secret_configured,revision,updated_at,updated_by FROM identity_external_provider WHERE provider_key=$1",
                vec![provider_key.as_str().into()],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(provider_admin_row)
            .transpose()?
            .ok_or(IdentityError::NotFound)
    }

    pub(crate) async fn write_identity_external(
        &mut self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        match command {
            IdentityWrite::CreateExternalFlow(flow) => {
                flow.validate(unsigned(at)?)?;
                self.identity_execute(
                    "INSERT INTO identity_external_flow (id,provider_key,purpose,state_digest,browser_binding_digest,user_id,oidc_flow_id,nonce,secret_ciphertext,created_at,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
                    vec![
                        flow.id.into(),
                        flow.provider_key.as_str().into(),
                        match flow.purpose {
                            ExternalPurpose::Login => "login",
                            ExternalPurpose::Bind => "bind",
                        }
                        .into(),
                        flow.state_digest.as_bytes().to_vec().into(),
                        flow.browser_binding_digest.as_bytes().to_vec().into(),
                        flow.user_id.into(),
                        flow.oidc_flow_id.into(),
                        flow.nonce.into(),
                        flow.secret_ciphertext.into(),
                        at.into(),
                        signed(flow.expires_at)?.into(),
                    ],
                )
                .await?;
                Ok(IdentityWriteResult::Resource(flow.id))
            }
            IdentityWrite::ConsumeExternalFlow { id } => {
                one_row(
                    self.identity_execute(
                        "UPDATE identity_external_flow SET consumed_at=$2 WHERE id=$1 AND consumed_at IS NULL AND expires_at>$2",
                        vec![id.into(), at.into()],
                    )
                    .await?,
                )?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CreateExternalIdentity(identity) => {
                if identity.id.is_nil()
                    || identity.user_id <= 0
                    || identity.subject.is_empty()
                    || identity.subject.len() > 255
                    || identity.created_at > unsigned(at)?
                {
                    return Err(IdentityError::InvalidInput);
                }
                self.identity_execute(
                    "INSERT INTO identity_external_identity (id,user_id,provider_key,provider_subject,provider_union_id,display_name,avatar_url,created_at,updated_at,last_login_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                    vec![
                        identity.id.into(),
                        identity.user_id.into(),
                        identity.provider_key.as_str().into(),
                        identity.subject.into(),
                        identity.union_id.into(),
                        identity.display_name.into(),
                        identity.avatar_url.into(),
                        signed(identity.created_at)?.into(),
                        signed(identity.updated_at)?.into(),
                        identity.last_login_at.map(signed).transpose()?.into(),
                    ],
                )
                .await?;
                Ok(IdentityWriteResult::Resource(identity.id))
            }
            IdentityWrite::DeleteExternalIdentity { id, user_id } => {
                one_row(
                    self.identity_execute(
                        "DELETE FROM identity_external_identity WHERE id=$1 AND user_id=$2",
                        vec![id.into(), user_id.into()],
                    )
                    .await?,
                )?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::UpdateExternalIdentityLastLogin { id, at } => {
                one_row(
                    self.identity_execute(
                        "UPDATE identity_external_identity SET last_login_at=$2,updated_at=$2 WHERE id=$1",
                        vec![id.into(), signed(at)?.into()],
                    )
                    .await?,
                )?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CreateExternalAccount(account) => {
                if account.username.is_empty()
                    || account.username.len() > 128
                    || account.nickname.is_empty()
                    || account.nickname.chars().count() > 200
                {
                    return Err(IdentityError::InvalidInput);
                }
                let role_id = crate::entity::role::RoleEnum::User.default_id();
                let role = self
                    .identity_one("SELECT name FROM role WHERE id=$1", vec![role_id.into()])
                    .await?;
                if get::<String>(&role, "name")? != crate::entity::role::RoleEnum::User.get_name() {
                    return Err(IdentityError::DependencyUnavailable);
                }
                let person_kind = match account.person_kind {
                    crate::domain::identity::account::PersonKind::Employee => "employee",
                    crate::domain::identity::account::PersonKind::Collaborator => "collaborator",
                };
                let user = self
                    .identity_one(
                        "INSERT INTO \"user\" (username,password,nickname,role_id,create_time,update_time,description,enabled,person_kind,developer_enabled,auth_epoch,identity_revision) VALUES ($1,NULL,$2,$3,$4,$4,NULL,TRUE,$5,FALSE,0,0) RETURNING id",
                        vec![
                            account.username.into(),
                            account.nickname.into(),
                            role_id.into(),
                            timestamp(at)?.into(),
                            person_kind.into(),
                        ],
                    )
                    .await?;
                let user_id: i32 = get(&user, "id")?;
                self.identity_execute(
                    "INSERT INTO identity_subject (issuer,sub,user_id,created_at) SELECT issuer,$1,$2,$3 FROM identity_provider WHERE id=1",
                    vec![user_id.to_string().into(), user_id.into(), at.into()],
                )
                .await?;
                let account = self
                    .identity_account(user_id)
                    .await?
                    .ok_or(IdentityError::DependencyUnavailable)?;
                Ok(IdentityWriteResult::Account(account))
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }
}

fn external_identity_row(
    row: sea_orm::QueryResult,
) -> Result<ExternalIdentityRecord, IdentityError> {
    Ok(ExternalIdentityRecord {
        id: get(&row, "id")?,
        user_id: get(&row, "user_id")?,
        provider_key: get::<String>(&row, "provider_key")?.parse()?,
        subject: get(&row, "provider_subject")?,
        display_name: get(&row, "display_name")?,
        avatar_url: get(&row, "avatar_url")?,
        created_at: unsigned(get(&row, "created_at")?)?,
        last_login_at: get::<Option<i64>>(&row, "last_login_at")?
            .map(unsigned)
            .transpose()?,
    })
}

fn provider_admin_row(
    row: sea_orm::QueryResult,
) -> Result<ExternalProviderAdminView, IdentityError> {
    let provider_key: String = get(&row, "provider_key")?;
    let parsed = provider_key.parse::<ExternalProviderKey>()?;
    let client_id: String = get(&row, "client_id")?;
    let secret_configured: bool = get(&row, "secret_configured")?;
    let enabled: bool = get(&row, "enabled")?;
    let config_complete = matches!(
        parsed,
        ExternalProviderKey::GitHub | ExternalProviderKey::Qq | ExternalProviderKey::WechatQr
    ) && !client_id.trim().is_empty()
        && secret_configured;
    Ok(ExternalProviderAdminView {
        provider_key,
        family: get(&row, "family")?,
        enabled,
        client_id,
        secret_configured,
        config_complete,
        revision: unsigned(get(&row, "revision")?)?,
        updated_at: unsigned(get(&row, "updated_at")?)? / 1000,
        updated_by: get(&row, "updated_by")?,
    })
}
