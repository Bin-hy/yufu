use super::repository::pg::{get, one_row, signed};
use super::repository::*;
use crate::domain::identity::{application::ClientKind, error::IdentityError};
use crate::service::security::repository::{PgSecurityTransaction, now, refresh_digest};
use auth::security::{credential::CredentialRef, identity::SecretToken};

impl PgSecurityTransaction {
    pub(crate) async fn identity_protocol_revocation_target(
        &self,
        issuer: &str,
        token: &SecretToken,
        access: Option<&CredentialRef>,
    ) -> Result<Option<ProtocolRevocationTarget>, IdentityError> {
        let (predicate, key) = match access {
            Some(reference) if reference.issuer() == issuer => (
                "c.credential_kind='access' AND c.jti=$2",
                reference.jti().into(),
            ),
            Some(_) => return Ok(None),
            None => (
                "c.credential_kind='refresh' AND c.secret_digest=$2",
                refresh_digest(token).to_vec().into(),
            ),
        };
        let rows = self.identity_rows(&format!("SELECT c.issuer,c.jti,c.grant_id,c.oidc_client_id,c.subject,s.environment_id,c.revoked_at IS NOT NULL OR g.revoked_at IS NOT NULL AS revoked FROM security_credential c JOIN security_session s ON s.id=c.session_id JOIN security_grant g ON g.id=c.grant_id WHERE c.issuer=$1 AND {predicate} AND c.oidc_client_id IS NOT NULL AND s.environment_id IS NOT NULL AND c.principal_kind='user' AND c.token_use='user_session' AND c.revocation_parent_jti IS NULL"), vec![issuer.into(), key]).await?;
        rows.first()
            .map(|row| {
                Ok(ProtocolRevocationTarget {
                    issuer: get(row, "issuer")?,
                    jti: get(row, "jti")?,
                    grant_id: get(row, "grant_id")?,
                    client_id: get(row, "oidc_client_id")?,
                    environment_id: get(row, "environment_id")?,
                    user_id: get::<String>(row, "subject")?
                        .parse()
                        .map_err(|_| IdentityError::DependencyUnavailable)?,
                    revoked: get(row, "revoked")?,
                })
            })
            .transpose()
    }
    pub(crate) async fn write_identity_reader(
        &mut self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        match command {
            IdentityWrite::CreateIntrospectionReader {
                id,
                reader_id,
                secret_hash,
                environment_id,
                approved_audience,
                actor_id,
            } => {
                let client = self
                    .environment_client(environment_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                let target = self.authorization_target(&client.client_id, None).await?;
                if !target.enabled
                    || target.config.client_kind != ClientKind::BrowserSpa
                    || target.config.access_audience.as_str() != approved_audience
                {
                    return Err(IdentityError::AccessDenied);
                }
                self.identity_execute("INSERT INTO identity_introspection_reader (id,reader_id,secret_hash,environment_id,approved_audience,enabled,secret_revision,created_by,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,TRUE,1,$6,$7,$7)",
                    vec![id.into(),reader_id.into(),secret_hash.expose_secret().into(),environment_id.into(),approved_audience.into(),actor_id.into(),at.into()]).await?;
                Ok(IdentityWriteResult::Resource(id))
            }
            IdentityWrite::UpdateIntrospectionReader {
                id,
                environment_id,
                expected_revision,
                enabled,
                secret_hash,
            } => {
                one_row(self.identity_execute("UPDATE identity_introspection_reader SET enabled=$4,secret_hash=COALESCE($5,secret_hash),secret_revision=secret_revision+1,updated_at=$6 WHERE id=$1 AND environment_id=$2 AND secret_revision=$3",
                    vec![id.into(),environment_id.into(),signed(expected_revision)?.into(),enabled.into(),secret_hash.map(|s|s.expose_secret().to_owned()).into(),at.into()]).await?)?;
                Ok(IdentityWriteResult::Revision(
                    expected_revision
                        .checked_add(1)
                        .ok_or(IdentityError::DependencyUnavailable)?,
                ))
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }
}
