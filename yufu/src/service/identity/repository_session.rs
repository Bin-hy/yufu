use super::repository::pg::{get, one_row, signed, timestamp};
use super::repository::*;
use crate::domain::identity::{
    account::SecretDigest,
    error::IdentityError,
    session::{FlowHandle, FlowPurpose, IdentityRevocationScope, MAX_PENDING_FLOWS},
};
use crate::service::security::repository::{PgSecurityTransaction, now};
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn identity_authorization_flow(
        &self,
        id: Uuid,
    ) -> Result<Option<crate::domain::identity::session::AuthorizationFlowSnapshot>, IdentityError>
    {
        use super::repository::pg::unsigned;
        use crate::domain::identity::session::AuthorizationFlowSnapshot;
        self.identity_rows(
            "SELECT * FROM identity_authorization_flow WHERE id=$1",
            vec![id.into()],
        )
        .await?
        .into_iter()
        .next()
        .map(|row| {
            Ok(AuthorizationFlowSnapshot {
                handle: FlowHandle {
                    id,
                    purpose: FlowPurpose::Authorization,
                    browser_proof_digest: SecretDigest::from_bytes(
                        get::<Vec<u8>>(&row, "browser_proof_digest")?
                            .try_into()
                            .map_err(|_| IdentityError::DependencyUnavailable)?,
                    ),
                    expires_at: unsigned(get(&row, "expires_at")?)?,
                    completed_at: get::<Option<i64>>(&row, "completed_at")?
                        .map(unsigned)
                        .transpose()?,
                },
                environment_id: get(&row, "environment_id")?,
                config_version_id: get(&row, "config_version_id")?,
                browser_binding_digest: SecretDigest::from_bytes(
                    get::<Vec<u8>>(&row, "browser_binding_digest")?
                        .try_into()
                        .map_err(|_| IdentityError::DependencyUnavailable)?,
                ),
                request_payload: get(&row, "request_payload")?,
                bound_sso_session_id: get(&row, "bound_sso_session_id")?,
                reauth_completed_at: get::<Option<i64>>(&row, "reauth_completed_at")?
                    .map(unsigned)
                    .transpose()?,
            })
        })
        .transpose()
    }
    pub(crate) async fn write_identity_session(
        &mut self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        match command {
            IdentityWrite::SaveConsent {
                user_id,
                environment_id,
                config_version_id,
                scopes,
            } => {
                self.save_identity_consent(user_id, environment_id, config_version_id, scopes, at)
                    .await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CreateCenter(root) => {
                let account = self
                    .identity_account(root.user_id)
                    .await?
                    .ok_or(IdentityError::Unauthenticated)?;
                root.validate(&account, super::repository::pg::unsigned(at)?)?;
                if root.revoked_at.is_some()
                    || root.expires_at - root.authenticated_at
                        > crate::domain::identity::session::DEFAULT_SSO_TTL_SECONDS * 1000
                {
                    return Err(IdentityError::InvalidInput);
                }
                self.identity_execute("INSERT INTO identity_sso_session (id,issuer,user_id,subject,secret_digest,auth_epoch,authenticated_at,expires_at,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",vec![root.id.into(),root.principal.issuer().into(),root.user_id.into(),root.principal.subject().into(),root.secret_digest.as_bytes().to_vec().into(),signed(root.auth_epoch)?.into(),signed(root.authenticated_at)?.into(),signed(root.expires_at)?.into(),at.into()]).await?;
                Ok(IdentityWriteResult::Resource(root.id))
            }
            IdentityWrite::AttachManagementSession {
                session_id,
                sso_session_id,
            } => {
                one_row(self.identity_execute("UPDATE security_session s SET sso_session_id=r.id,auth_epoch=r.auth_epoch FROM identity_sso_session r JOIN \"user\" u ON u.id=r.user_id WHERE s.id=$1 AND r.id=$2 AND s.principal_kind='user' AND s.issuer=r.issuer AND s.subject=r.subject AND s.sso_session_id IS NULL AND s.revoked_at IS NULL AND r.revoked_at IS NULL AND r.expires_at>$3 AND r.auth_epoch=u.auth_epoch AND u.enabled AND NOT EXISTS (SELECT 1 FROM security_credential c WHERE c.session_id=s.id AND c.expires_at>r.expires_at)",vec![session_id.into(),sso_session_id.into(),at.into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::AttachIdentitySession {
                session_id,
                rp_session_id,
                admission,
            } => {
                self.require_current_admission(&admission).await?;
                one_row(self.identity_execute("UPDATE security_session s SET sso_session_id=r.sso_session_id,oidc_rp_session_id=r.id,environment_id=r.environment_id,config_version_id=r.config_version_id,auth_epoch=$4,lifecycle_epoch=$5,access_generation=$6,identity_scopes=r.identity_scopes FROM identity_oidc_rp_session r JOIN identity_sso_session root ON root.id=r.sso_session_id JOIN oauth_client client ON client.id=r.oauth_client_id WHERE s.id=$1 AND r.id=$2 AND r.environment_id=$3 AND s.sso_session_id IS NULL AND s.revoked_at IS NULL AND s.issuer=r.issuer AND s.subject=r.subject AND s.principal_kind='user' AND r.revoked_at IS NULL AND r.config_version_id=$7 AND r.sso_session_id=$8 AND r.identity_scopes=$9 AND root.revoked_at IS NULL AND NOT EXISTS (SELECT 1 FROM security_credential c WHERE c.session_id=s.id AND (c.oidc_client_id IS DISTINCT FROM client.client_id OR c.expires_at>root.expires_at OR c.bounds<>'{\"grants\":[]}'::jsonb))", vec![session_id.into(),rp_session_id.into(),admission.environment_id().into(),signed(admission.auth_epoch())?.into(),signed(admission.lifecycle_epoch())?.into(),signed(admission.access_generation())?.into(),admission.config_version_id().into(),admission.sso_session_id().into(),admission.scopes().as_protocol_string().into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CreateFlow {
                flow,
                interactive,
                environment_id,
                config_version_id,
                browser_binding_digest,
                request_payload,
            } => {
                if flow.purpose != FlowPurpose::Authorization
                    || flow.completed_at.is_some()
                    || flow.expires_at <= super::repository::pg::unsigned(at)?
                    || flow.expires_at - super::repository::pg::unsigned(at)?
                        > crate::domain::identity::session::DEFAULT_FLOW_TTL_SECONDS * 1000
                {
                    return Err(IdentityError::InvalidInput);
                }
                if interactive {
                    // A row write, rather than just an advisory lock, forces a stale
                    // REPEATABLE READ snapshot to retry before enforcing the quota.
                    self.identity_execute("INSERT INTO identity_browser_binding (secret_digest,touched_at) VALUES ($1,$2) ON CONFLICT (secret_digest) DO UPDATE SET revision=identity_browser_binding.revision+1,touched_at=EXCLUDED.touched_at",vec![browser_binding_digest.as_bytes().to_vec().into(),at.into()]).await?;
                    let row = self.identity_one("SELECT COUNT(*)::bigint AS pending FROM identity_authorization_flow WHERE browser_binding_digest=$1 AND completed_at IS NULL AND expires_at>$2",vec![browser_binding_digest.as_bytes().to_vec().into(),at.into()]).await?;
                    if get::<i64>(&row, "pending")? >= MAX_PENDING_FLOWS as i64 {
                        return Err(IdentityError::CapacityExceeded);
                    }
                }
                self.identity_execute("INSERT INTO identity_authorization_flow (id,request_payload,environment_id,config_version_id,browser_proof_digest,browser_binding_digest,created_at,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",vec![flow.id.into(),request_payload.into(),environment_id.into(),config_version_id.into(),flow.browser_proof_digest.as_bytes().to_vec().into(),browser_binding_digest.as_bytes().to_vec().into(),at.into(),signed(flow.expires_at)?.into()]).await?;
                Ok(IdentityWriteResult::Resource(flow.id))
            }
            IdentityWrite::BindFlow {
                flow_id,
                sso_session_id,
                reauthenticated,
            } => {
                one_row(self.identity_execute("UPDATE identity_authorization_flow SET bound_sso_session_id=$2,reauth_completed_at=CASE WHEN $3 THEN $4 ELSE reauth_completed_at END WHERE id=$1 AND completed_at IS NULL AND expires_at>$4",vec![flow_id.into(),sso_session_id.into(),reauthenticated.into(),at.into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CompleteFlow { flow_id } => {
                one_row(self.identity_execute("UPDATE identity_authorization_flow SET completed_at=$2 WHERE id=$1 AND completed_at IS NULL AND expires_at>$2",vec![flow_id.into(),at.into()]).await?)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::CreateRelyingParty {
                id,
                protocol_sid,
                admission,
                nonce,
            } => {
                self.require_current_admission(&admission).await?;
                self.identity_execute("INSERT INTO identity_oidc_rp_session (id,sso_session_id,issuer,environment_id,config_version_id,protocol_sid,subject,auth_time,nonce,identity_scopes,created_at,oauth_client_id) SELECT $1,$2,$3,e.id,$5,$6,$7,$8,$9,$10,$11,e.oauth_client_id FROM identity_app_environment e WHERE e.id=$4 AND e.active_version_id=$5",vec![id.into(),admission.sso_session_id().into(),admission.principal().issuer().into(),admission.environment_id().into(),admission.config_version_id().into(),protocol_sid.into(),admission.principal().subject().into(),signed(admission.auth_time())?.into(),nonce.into(),admission.scopes().as_protocol_string().into(),at.into()]).await.and_then(one_row)?;
                Ok(IdentityWriteResult::Resource(id))
            }
            IdentityWrite::CreateCode {
                digest,
                flow_id,
                rp_session_id,
                admission,
                verified_request,
                expires_at,
            } => {
                self.require_current_admission(&admission).await?;
                let env = self.identity_one("SELECT c.client_id,r.expires_at FROM identity_app_environment e JOIN oauth_client c ON c.id=e.oauth_client_id JOIN identity_sso_session r ON r.id=$2 WHERE e.id=$1",vec![admission.environment_id().into(),admission.sso_session_id().into()]).await?;
                if expires_at <= super::repository::pg::unsigned(at)?
                    || expires_at > super::repository::pg::unsigned(get(&env, "expires_at")?)?
                    || expires_at - super::repository::pg::unsigned(at)?
                        > crate::domain::identity::session::DEFAULT_CODE_TTL_SECONDS * 1000
                {
                    return Err(IdentityError::InvalidInput);
                }
                let config = self
                    .identity_version_config(admission.config_version_id(), true)
                    .await?;
                let request =
                    crate::domain::identity::authorization::AuthorizationParameters::from_payload(
                        verified_request.clone(),
                    )?
                    .validate()?;
                if !config.accepts_redirect(&request.redirect_uri)
                    || request.client_id != get::<String>(&env, "client_id")?
                    || request.scopes != *admission.scopes()
                {
                    return Err(IdentityError::InvalidInput);
                }
                let user_id = admission
                    .principal()
                    .subject()
                    .parse::<i32>()
                    .map_err(|_| IdentityError::InvalidInput)?;
                let code_challenge = request.code_challenge.clone().unwrap_or_default();
                let code_challenge_method = if request.code_challenge.is_some() {
                    "S256"
                } else {
                    ""
                };
                self.identity_execute("INSERT INTO oauth_authorization_code (code,user_id,client_id,redirect_uri,code_challenge,code_challenge_method,scope,state,nonce,used,expires_at,create_time,code_digest,flow_id,sso_session_id,environment_id,config_version_id,auth_time,auth_epoch,lifecycle_epoch,access_generation,oidc_rp_session_id) SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,FALSE,$10,$11,$12,f.id,$14,$15,$16,$17,$18,$19,$20,r.id FROM identity_authorization_flow f JOIN identity_oidc_rp_session r ON r.id=$22 AND r.sso_session_id=$14 AND r.environment_id=$15 AND r.config_version_id=$16 AND r.subject=$2::text AND r.auth_time=$17 AND r.nonce IS NOT DISTINCT FROM $9 AND r.identity_scopes=$7 AND r.revoked_at IS NULL JOIN oauth_client c ON c.id=r.oauth_client_id AND c.client_id=$3 WHERE f.id=$13 AND f.bound_sso_session_id=$14 AND f.environment_id=$15 AND f.config_version_id=$16 AND f.completed_at IS NULL AND f.expires_at>$21 AND f.request_payload=$23",vec![format!("sha256:{}",digest.hex()).into(),user_id.into(),request.client_id.into(),request.redirect_uri.into(),code_challenge.into(),code_challenge_method.into(),admission.scopes().as_protocol_string().into(),request.state.into(),request.nonce.into(),timestamp(signed(expires_at)?)?.into(),timestamp(at)?.into(),digest.as_bytes().to_vec().into(),flow_id.into(),admission.sso_session_id().into(),admission.environment_id().into(),admission.config_version_id().into(),signed(admission.auth_time())?.into(),signed(admission.auth_epoch())?.into(),signed(admission.lifecycle_epoch())?.into(),signed(admission.access_generation())?.into(),at.into(),rp_session_id.into(),verified_request.into()]).await.and_then(one_row)?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::Revoke(scope) => {
                self.revoke_identity_scope(&scope, at).await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::RevokeAccountSessions { user_id } => {
                self.revoke_identity_scope_with_epoch(
                    &IdentityRevocationScope::UserAll { user_id },
                    at,
                    false,
                )
                .await?;
                Ok(IdentityWriteResult::Changed)
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }
    async fn require_current_admission(
        &self,
        admission: &crate::domain::identity::admission::ManagedAdmission,
    ) -> Result<(), IdentityError> {
        let current = self
            .current_identity_admission(&AdmissionQuery {
                sso_session_id: admission.sso_session_id(),
                environment_id: admission.environment_id(),
                requested_scopes: admission.scopes().clone(),
            })
            .await?;
        if !admission.still_current(&current) {
            return Err(IdentityError::AccessDenied);
        }
        Ok(())
    }
    pub(crate) async fn revoke_identity_scope(
        &self,
        scope: &IdentityRevocationScope,
        at: i64,
    ) -> Result<(), IdentityError> {
        self.revoke_identity_scope_with_epoch(scope, at, true).await
    }
    async fn revoke_identity_scope_with_epoch(
        &self,
        scope: &IdentityRevocationScope,
        at: i64,
        advance_epoch: bool,
    ) -> Result<(), IdentityError> {
        let (predicate, target, user): (&str, sea_orm::Value, Option<i32>) = match scope {
            IdentityRevocationScope::CurrentCenter { sso_session_id } => {
                ("s.sso_session_id=$1", (*sso_session_id).into(), None)
            }
            IdentityRevocationScope::UserAll { user_id } => (
                "s.principal_kind='user' AND s.subject=$1",
                user_id.to_string().into(),
                Some(*user_id),
            ),
            IdentityRevocationScope::ApplicationLocal { rp_session_id } => {
                ("s.oidc_rp_session_id=$1", (*rp_session_id).into(), None)
            }
            IdentityRevocationScope::Environment { environment_id } => {
                ("s.environment_id=$1", (*environment_id).into(), None)
            }
            IdentityRevocationScope::EnvironmentUser {
                environment_id,
                user_id,
            } => (
                "s.environment_id=$1 AND s.subject=$3::text",
                (*environment_id).into(),
                Some(*user_id),
            ),
            IdentityRevocationScope::Grant { grant_id } => (
                "s.id IN (SELECT c.session_id FROM security_credential c WHERE c.grant_id=$1)",
                (*grant_id).into(),
                None,
            ),
        };
        // Scope selectors are closed server enums. The same selected roots are
        // used for grants, families and recursive descendants (including MCP).
        let mut params = vec![target.clone(), at.into()];
        if matches!(scope, IdentityRevocationScope::EnvironmentUser { .. }) {
            params.push(user.into());
        }
        let selected = format!("SELECT s.id FROM security_session s WHERE {predicate}");
        self.identity_execute(&format!("UPDATE security_grant SET revoked_at=COALESCE(revoked_at,$2) WHERE id IN (SELECT c.grant_id FROM security_credential c WHERE c.session_id IN ({selected}))"),params.clone()).await?;
        self.identity_execute(&format!("UPDATE security_session SET revoked_at=COALESCE(revoked_at,$2) WHERE id IN ({selected})"),params.clone()).await?;
        self.identity_execute(&format!("UPDATE security_refresh_family SET revoked_at=COALESCE(revoked_at,$2) WHERE id IN (SELECT c.family_id FROM security_credential c WHERE c.session_id IN ({selected}))"),params.clone()).await?;
        self.identity_execute(&format!("WITH RECURSIVE revoked(issuer,jti) AS (SELECT c.issuer,c.jti FROM security_credential c WHERE c.session_id IN ({selected}) UNION SELECT c.issuer,c.jti FROM security_credential c JOIN revoked p ON c.issuer=p.issuer AND c.revocation_parent_jti=p.jti) UPDATE security_credential c SET revoked_at=COALESCE(c.revoked_at,$2) FROM revoked r WHERE c.issuer=r.issuer AND c.jti=r.jti"),params.clone()).await?;
        let (rp_predicate, code_predicate) = match scope {
            IdentityRevocationScope::CurrentCenter { .. } => {
                (Some("sso_session_id=$1"), Some("sso_session_id=$1"))
            }
            IdentityRevocationScope::UserAll { .. } => {
                (Some("subject=$1"), Some("user_id::text=$1"))
            }
            IdentityRevocationScope::Environment { .. } => {
                (Some("environment_id=$1"), Some("environment_id=$1"))
            }
            IdentityRevocationScope::EnvironmentUser { .. } => (
                Some("environment_id=$1 AND subject=$3::text"),
                Some("environment_id=$1 AND user_id=$3"),
            ),
            IdentityRevocationScope::ApplicationLocal { .. } => (Some("id=$1"), None),
            IdentityRevocationScope::Grant { .. } => (
                Some(
                    "id IN (SELECT s.oidc_rp_session_id FROM security_credential c JOIN security_session s ON s.id=c.session_id WHERE c.grant_id=$1)",
                ),
                None,
            ),
        };
        if let Some(predicate) = rp_predicate {
            self.identity_execute(&format!("UPDATE identity_oidc_rp_session SET revoked_at=COALESCE(revoked_at,$2) WHERE {predicate}"),params.clone()).await?;
        }
        if let Some(predicate) = code_predicate {
            self.identity_execute(&format!("UPDATE oauth_authorization_code SET used=TRUE WHERE {predicate} AND code_digest IS NOT NULL AND $2::bigint>=0"),params.clone()).await?;
        }
        match scope {
            IdentityRevocationScope::CurrentCenter { .. } => {
                self.identity_execute("UPDATE identity_sso_session SET revoked_at=COALESCE(revoked_at,$2) WHERE id=$1",params).await?;
            }
            IdentityRevocationScope::UserAll { user_id } => {
                if advance_epoch {
                    self.identity_execute("UPDATE \"user\" SET auth_epoch=auth_epoch+1,identity_revision=identity_revision+1 WHERE id=$1",vec![(*user_id).into()]).await?;
                }
                self.identity_execute("UPDATE identity_sso_session SET revoked_at=COALESCE(revoked_at,$2) WHERE user_id=$1",vec![(*user_id).into(),at.into()]).await?;
            }
            _ => {}
        }
        Ok(())
    }
}
