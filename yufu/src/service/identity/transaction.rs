use super::repository::{IdentityRepository, IdentityTransaction};
use crate::domain::identity::error::IdentityError;
use auth::security::budget::SecurityBudget;

/// The input is stable across retries. Only a committed attempt's output may
/// leave this boundary; uncertain commits must never repeat secret issuance.
#[async_trait::async_trait]
#[allow(clippy::double_must_use)]
pub trait IdentityUnitOfWork: Send + Sync {
    type Output: Send;
    async fn apply(
        &self,
        transaction: &mut dyn IdentityTransaction,
    ) -> Result<Self::Output, IdentityError>;
}

pub async fn execute_identity<U: IdentityUnitOfWork>(
    repository: &dyn IdentityRepository,
    budget: &SecurityBudget,
    unit: &U,
) -> Result<U::Output, IdentityError> {
    for attempt in 0..3 {
        budget.remaining()?;
        let mut transaction = match repository.begin(budget).await {
            Ok(transaction) => transaction,
            Err(IdentityError::Conflict) if attempt < 2 => continue,
            Err(error) => return Err(error),
        };
        let output = match unit.apply(transaction.as_mut()).await {
            Ok(output) => output,
            Err(error) => {
                let _ = transaction.rollback().await;
                if error == IdentityError::Conflict {
                    budget.remaining()?;
                    if attempt < 2 {
                        continue;
                    }
                }
                return Err(error);
            }
        };
        match transaction.commit().await.map_err(IdentityError::from) {
            Ok(()) => return Ok(output),
            Err(IdentityError::Conflict) if attempt < 2 => {}
            Err(error) => return Err(error),
        }
    }
    Err(IdentityError::Conflict)
}
