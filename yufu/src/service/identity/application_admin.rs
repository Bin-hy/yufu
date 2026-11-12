use super::*;
use crate::domain::identity::{operation::IdentityOperation, registration::CreateAdminApplication};

impl ApplicationService {
    pub async fn register_administrator(
        &self,
        evidence: &BrowserWriteEvidence,
        request: CreateAdminApplication,
        key: &str,
        request_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<RegistrationReceipt, IdentityError> {
        validate_idempotency_key(key)?;
        let request = request.validate()?;
        let mut tx = self.repository.begin(budget).await?;
        let existing = async {
            let actor = read_current_actor(
                tx.as_mut(),
                evidence
                    .current_center_digest
                    .as_ref()
                    .ok_or(IdentityError::Unauthenticated)?,
                now(self.clock.as_ref())?,
            )
            .await?;
            tx.authorize_identity_operation(&actor, IdentityOperation::ApplicationRegister)
                .await?;
            if !request.assignments.is_empty() {
                tx.authorize_identity_operation(&actor, IdentityOperation::ApplicationAssignment)
                    .await?;
            }
            tx.operation_receipt(actor.account().user_id, "application.admin.register", key)
                .await
        }
        .await;
        if let Some(receipt) = finish_read(tx, existing, budget).await? {
            return registration_from_receipt(receipt, &request.application.request_hash);
        }
        let (development, production) = tokio::try_join!(
            PreparedEnvironment::new(&request.application.development),
            PreparedEnvironment::new(&request.application.production)
        )?;
        let write_budget = budget.renewed()?;
        let outcome = execute_identity(
            self.repository.as_ref(),
            &write_budget,
            &Register {
                service: self,
                evidence,
                request: &request.application,
                key,
                request_id,
                operation_id: Uuid::new_v4(),
                development: &development,
                production: &production,
                administrator: Some(&request),
            },
        )
        .await?;
        let mut receipt =
            registration_from_receipt(outcome.receipt, &request.application.request_hash)?;
        if outcome.created {
            receipt.newly_issued_secrets = [development, production]
                .into_iter()
                .filter_map(|env| env.secret.map(|secret| (env.id, secret)))
                .collect();
        }
        Ok(receipt)
    }
}
