//! Managed refresh shares the identity transaction; this module never commits.
use super::*;
use crate::domain::identity::session::IdentityTokenContext;
use uuid::Uuid;

pub(crate) struct IdentityAccess {
    pub(crate) context: Option<IdentityTokenContext>,
    pub(crate) subject: String,
    pub(crate) validity: DecisionValidity,
    pub(crate) audience: String,
    pub(crate) expires_at: u64,
    pub(crate) client_id: String,
}

pub(crate) struct ManagedRefresh {
    snapshot: CredentialSnapshot,
    context: IdentityTokenContext,
}
impl ManagedRefresh {
    pub(crate) fn context(&self) -> &IdentityTokenContext {
        &self.context
    }
    pub(crate) fn grant_id(&self) -> Uuid {
        self.snapshot.current.grant.id
    }
    pub(crate) fn replayed(&self) -> bool {
        self.snapshot.current.credential.consumed_at.is_some()
    }
    pub(crate) fn same_binding(&self, other: &Self) -> bool {
        self.snapshot.reference() == other.snapshot.reference()
            && self.context.admission == other.context.admission
            && self.context.client_id == other.context.client_id
            && self.context.protocol_sid == other.context.protocol_sid
            && self.snapshot.current.credential.grant_id
                == other.snapshot.current.credential.grant_id
            && self.snapshot.current.credential.session_id
                == other.snapshot.current.credential.session_id
            && self.snapshot.current.credential.family_id
                == other.snapshot.current.credential.family_id
    }
}
impl CredentialLifecycle {
    pub(crate) async fn identity_access_in_transaction(
        &self,
        txn: &mut dyn SecurityTransaction,
        token: &SecretToken,
        issuer: &str,
        verifier: &auth::token_manager::IdTokenSigner,
    ) -> Result<IdentityAccess, SecurityError> {
        use auth::security::credential::CredentialExpectation;
        // First verify the signed reference for lookup. The expectations for
        // the full signature/claim check then come from trusted registration.
        let reference = verifier.revocation_reference(token, issuer)?;
        let snapshot = txn
            .credential(&reference)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let row = &snapshot.current.credential;
        let client_id = row
            .oidc_client_id
            .as_deref()
            .ok_or(SecurityError::InvalidCredential)?;
        let client = txn
            .oidc_client(client_id)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let binding = txn
            .session_client(issuer, client_id)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let expected = CredentialExpectation::new(
            issuer,
            binding.audience,
            binding.application,
            vec![CredentialProfile::new(
                PrincipalKind::User,
                TokenUse::UserSession,
            )?],
        )?;
        let verified =
            verifier.verify_security_credential(token, &expected, self.clock.as_ref())?;
        let (status, key) = status_for_snapshot(&snapshot, now(self.clock.as_ref())?)?;
        if &status.claims != verified.claims() || key != verified.signing_key_id() {
            return Err(SecurityError::InvalidCredential);
        }
        let context = snapshot
            .current
            .identity
            .as_ref()
            .and_then(|state| state.managed.clone());
        match context.as_ref() {
            Some(context)
                if client.management_state == "managed" && context.client_id == client_id => {}
            None if client.management_state == "legacy_unmanaged" => {}
            _ => return Err(SecurityError::InvalidCredential),
        }
        Ok(IdentityAccess {
            context,
            subject: status.claims.sub,
            validity: status.validity,
            audience: status.claims.aud,
            expires_at: status.claims.exp,
            client_id: client_id.into(),
        })
    }
    pub(crate) async fn identity_protocol_token_in_transaction(
        &self,
        txn: &mut dyn SecurityTransaction,
        token: &SecretToken,
        issuer: &str,
        verifier: &auth::token_manager::IdTokenSigner,
    ) -> Result<IdentityAccess, SecurityError> {
        if token.expose_secret().split('.').count() == 3 {
            return self
                .identity_access_in_transaction(txn, token, issuer, verifier)
                .await;
        }
        let snapshot = txn
            .refresh(issuer, token)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        validate_refresh_roots(&snapshot.current)?;
        let row = &snapshot.current.credential;
        let client_id = row
            .oidc_client_id
            .as_deref()
            .ok_or(SecurityError::InvalidCredential)?;
        let client = txn
            .oidc_client(client_id)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        let binding = txn
            .session_client(issuer, client_id)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        if row.issuer != issuer
            || row.audience != binding.audience
            || row.application != binding.application
            || row.principal_kind != "user"
            || row.token_use != "user_session"
            || row.revocation_parent_jti.is_some()
        {
            return Err(SecurityError::InvalidCredential);
        }
        let context = snapshot
            .current
            .identity
            .as_ref()
            .and_then(|state| state.managed.clone());
        match context.as_ref() {
            Some(context)
                if client.management_state == "managed" && context.client_id == client_id => {}
            None if client.management_state == "legacy_unmanaged" => {}
            _ => return Err(SecurityError::InvalidCredential),
        }
        let mut access_state = snapshot.current.clone();
        access_state.credential.credential_kind = "access".into();
        let claims = active_claims(&access_state, now(self.clock.as_ref())?)?;
        let validity = DecisionValidity::from_query_start(
            snapshot.checked_at,
            u64::try_from(row.expires_at).map_err(|_| SecurityError::DependencyUnavailable)?,
            snapshot
                .current
                .identity
                .as_ref()
                .map(|s| s.root_expires_at),
            None,
        )?;
        Ok(IdentityAccess {
            context,
            subject: claims.sub,
            validity,
            audience: claims.aud,
            expires_at: claims.exp,
            client_id: client_id.into(),
        })
    }
    pub(crate) async fn load_identity_refresh_in_transaction(
        &self,
        txn: &mut dyn SecurityTransaction,
        request: &RefreshRequest<'_>,
        issuer: &str,
        locked: bool,
    ) -> Result<ManagedRefresh, SecurityError> {
        let snapshot = if locked {
            txn.lock_refresh(issuer, request.refresh_token).await?
        } else {
            txn.refresh(issuer, request.refresh_token).await?
        }
        .ok_or(SecurityError::InvalidCredential)?;
        validate_refresh_roots(&snapshot.current)?;
        let row = &snapshot.current.credential;
        let context = snapshot
            .current
            .identity
            .as_ref()
            .and_then(|state| state.managed.as_ref())
            .ok_or(SecurityError::InvalidCredential)?
            .clone();
        if row.issuer != issuer
            || row.oidc_client_id.as_deref() != Some(request.client_id)
            || context.client_id != request.client_id
            || row.principal_kind != "user"
            || row.token_use != "user_session"
            || row.revocation_parent_jti.is_some()
            || !decode::<CredentialBounds>(row.bounds.clone())?
                .grants()
                .is_empty()
        {
            return Err(SecurityError::InvalidCredential);
        }
        let binding = txn
            .session_client(issuer, request.client_id)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        if row.audience != binding.audience || row.application != binding.application {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(ManagedRefresh { snapshot, context })
    }

    pub(crate) async fn rotate_identity_refresh_in_transaction(
        &self,
        txn: &mut dyn SecurityTransaction,
        refresh: &ManagedRefresh,
        profile: &auth::claims::IdentityProfile,
        access_ttl_seconds: u64,
        signer: &dyn SessionTokenSigner,
        budget: &SecurityBudget,
    ) -> Result<SessionTokenSet, SecurityError> {
        use rand::RngCore;
        let snapshot = &refresh.snapshot;
        let row = &snapshot.current.credential;
        let at = now(self.clock.as_ref())?;
        let mut access_state = snapshot.current.clone();
        access_state.credential.credential_kind = "access".into();
        let mut claims = active_claims(&access_state, at)?;
        let paired = CredentialRef::new(
            &row.issuer,
            row.paired_access_jti
                .as_deref()
                .ok_or(SecurityError::InvalidCredential)?,
        )?;
        let paired_snapshot = txn
            .credential(&paired)
            .await?
            .ok_or(SecurityError::InvalidCredential)?;
        validate_session_access(row, &paired_snapshot.current.credential)?;
        txn.consume_refresh_pair(snapshot.reference(), &paired, at)
            .await?;
        claims.jti = Uuid::new_v4().to_string();
        claims.iat = at / 1000;
        let refresh_expiry =
            u64::try_from(row.expires_at).map_err(|_| SecurityError::InvalidCredential)?;
        claims.exp = refresh
            .context
            .expiry_seconds(claims.iat, access_ttl_seconds)?
            .min(refresh_expiry / 1000);
        claims.nbf = None;
        let mut random = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut random);
        let refresh_token = SecretToken::new(hex::encode(random))?;
        let access_token = budget.run(signer.sign_access(&claims)).await??;
        // OIDC Core 12.2: the original authentication time is retained; nonce
        // is omitted from refresh ID Tokens, rather than inventing a new one.
        let mut context = refresh.context.clone();
        context.nonce = None;
        let id_token = budget
            .run(signer.sign_identity_id(&claims, &context, profile, &access_token))
            .await??;
        let replacement = Uuid::new_v4().to_string();
        txn.register_session_pair(SessionRegistration {
            client_id: Some(context.client_id),
            claims: claims.clone(),
            key_id: signer.key_id().into(),
            refresh_jti: replacement.clone(),
            refresh_digest: refresh_digest(&refresh_token),
            refresh_expires_at: refresh_expiry,
            minted_by: Some(snapshot.reference().clone()),
            new_roots: false,
        })
        .await?;
        txn.link_refresh_replacement(snapshot.reference(), &replacement)
            .await?;
        budget.remaining()?;
        Ok(SessionTokenSet {
            access_token,
            id_token: Some(id_token),
            refresh_token,
            claims,
        })
    }

    pub(crate) async fn revoke_identity_refresh_replay_in_transaction(
        &self,
        txn: &mut dyn SecurityTransaction,
        refresh: &ManagedRefresh,
    ) -> Result<(), SecurityError> {
        if !refresh.replayed() {
            return Err(SecurityError::InvalidCredential);
        }
        txn.revoke_refresh_family(refresh.snapshot.reference(), now(self.clock.as_ref())?)
            .await
    }
}
