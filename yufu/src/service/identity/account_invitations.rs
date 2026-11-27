use super::{AccountService, IssuedAccountAction, actions::audit, authenticate_current_actor};
use crate::domain::identity::{
    account::{InvitationPage, InvitationView, IssuedSecret, SecretDigest, ensure_enabled},
    error::IdentityError,
    operation::IdentityOperation,
};
use crate::service::identity::{
    browser::BrowserWriteEvidence,
    repository::*,
    search, sso,
    transaction::{IdentityUnitOfWork, execute_identity},
};
use crate::service::security::repository::now;
use auth::security::budget::SecurityBudget;
use uuid::Uuid;

impl AccountService {
    pub async fn list_invitations(
        &self,
        secret: &str,
        mut query: crate::domain::identity::account::InvitationListQuery,
        budget: &SecurityBudget,
    ) -> Result<InvitationPage, IdentityError> {
        if !(1..=50).contains(&query.limit) {
            return Err(IdentityError::InvalidInput);
        }
        query.search = search::normalize_query(query.search.as_deref())?;
        query.now_millis = now(self.clock.as_ref())?;
        sso::validate_opaque(secret)?;
        let mut transaction = self.repository.begin(budget).await?;
        let result = async {
            let actor = super::read_current_actor(
                transaction.as_mut(),
                &SecretDigest::of(secret),
                query.now_millis,
            )
            .await?;
            if !actor.account().administrator {
                return Err(IdentityError::Forbidden);
            }
            transaction
                .authorize_identity_operation(&actor, IdentityOperation::AccountRead)
                .await?;
            let limit = query.limit as usize;
            let mut rows = transaction.list_invitations(query).await?;
            let has_more = rows.len() > limit;
            rows.truncate(limit);
            let next = has_more
                .then(|| rows.last())
                .flatten()
                .map(|row| format!("{}:{}", row.created_at_millis, row.id));
            Ok(InvitationPage {
                items: rows.into_iter().map(view).collect(),
                next,
            })
        }
        .await;
        transaction.rollback().await?;
        result
    }

    pub async fn reissue_invitation(
        &self,
        evidence: &BrowserWriteEvidence,
        source_id: Uuid,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<IssuedAccountAction, IdentityError> {
        if source_id.is_nil() {
            return Err(IdentityError::InvalidInput);
        }
        execute_identity(
            self.repository.as_ref(),
            budget,
            &ReissueInvitation {
                service: self,
                evidence,
                source_id,
                request_id,
            },
        )
        .await
    }
}

fn view(row: crate::domain::identity::account::InvitationRow) -> InvitationView {
    InvitationView {
        id: row.id,
        purpose: row.purpose,
        target_user_id: row.target_user_id,
        target_username: row.target_username,
        target_nickname: row.target_nickname,
        person_kind: row.person_kind,
        developer_grant: row.developer_grant,
        created_by: row.created_by,
        creator_username: row.creator_username,
        creator_nickname: row.creator_nickname,
        created_at: row.created_at_millis / 1_000,
        expires_at: row.expires_at_millis / 1_000,
        status: row.status,
        accepted_user_id: row.accepted_user_id,
        accepted_username: row.accepted_username,
        accepted_nickname: row.accepted_nickname,
        accepted_at: row.accepted_at_millis.map(|value| value / 1_000),
        revoked_at: row.revoked_at_millis.map(|value| value / 1_000),
    }
}

struct ReissueInvitation<'a> {
    service: &'a AccountService,
    evidence: &'a BrowserWriteEvidence,
    source_id: Uuid,
    request_id: Uuid,
}

#[async_trait::async_trait]
impl IdentityUnitOfWork for ReissueInvitation<'_> {
    type Output = IssuedAccountAction;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError> {
        let at = now(self.service.clock.as_ref())?;
        let source = transaction
            .load_invitation(self.source_id)
            .await?
            .ok_or(IdentityError::NotFound)?;
        let mut keys = vec![IdentityLockKey::Invitation(self.source_id)];
        if let Some(user_id) = source.target_user_id {
            keys.push(IdentityLockKey::User(user_id));
        }
        let scope = IdentityLockScope::new(keys)?;
        let digest = self
            .evidence
            .current_center_digest
            .as_ref()
            .ok_or(IdentityError::Unauthenticated)?;
        let actor = authenticate_current_actor(transaction, digest, at, &scope).await?;
        transaction
            .authorize_identity_operation(&actor, IdentityOperation::InvitationIssue)
            .await?;
        if let Some(user_id) = source.target_user_id {
            let account = transaction
                .load_account(user_id)
                .await?
                .ok_or(IdentityError::NotFound)?;
            ensure_enabled(&account)?;
            if account.person_kind != source.person_kind {
                return Err(IdentityError::InvalidInput);
            }
        } else if source.purpose
            == crate::domain::identity::account::InvitationPurpose::Qualification
        {
            return Err(IdentityError::InvalidInput);
        }
        let secret = IssuedSecret::from_entropy(rand::random());
        let id = Uuid::new_v4();
        let expires_at = at
            .checked_add(self.service.config.identity.invitation_ttl_seconds * 1_000)
            .ok_or(IdentityError::CapacityExceeded)?;
        transaction
            .apply(IdentityWrite::IssueInvitation(InvitationWrite {
                id,
                digest: secret.digest(),
                purpose: source.purpose,
                target_user_id: source.target_user_id,
                person_kind: source.person_kind,
                developer_grant: source.developer_grant,
                created_by: actor.account().user_id,
                expires_at,
            }))
            .await?;
        audit(
            transaction,
            Some(actor.account().user_id),
            self.request_id,
            "account.invitation.reissue",
            "invitation",
            id.to_string(),
            serde_json::json!({
                "invitation_id": source.id,
                "target_user_id": source.target_user_id,
                "expires_at": expires_at
            }),
        )
        .await?;
        Ok(IssuedAccountAction {
            id,
            secret,
            expires_at,
        })
    }
}
