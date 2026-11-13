use super::{
    account::read_current_actor, application::finish_read, repository::IdentityRepository,
};
use crate::{
    domain::identity::{
        account::SecretDigest,
        audit::{AuditCursor, AuditFilterInput, AuditPage},
        console::validate_limit,
        error::IdentityError,
        operation::IdentityOperation,
    },
    service::security::repository::now,
};
use auth::security::budget::{Clock, SecurityBudget};
use std::sync::Arc;

pub struct AuditService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
}
impl AuditService {
    pub fn new(repository: Arc<dyn IdentityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn search(
        &self,
        secret: &str,
        input: AuditFilterInput,
        cursor: Option<&str>,
        limit: u32,
        budget: &SecurityBudget,
    ) -> Result<AuditPage, IdentityError> {
        validate_limit(limit)?;
        let filter = input.validate()?;
        let cursor = cursor
            .map(|cursor| AuditCursor::decode(cursor, &filter))
            .transpose()?;
        super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            if !actor.account().administrator {
                return Err(IdentityError::Forbidden);
            }
            tx.authorize_identity_operation(&actor, IdentityOperation::AuditRead)
                .await?;
            tx.audit_search(&filter, cursor.as_ref(), limit).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
}
