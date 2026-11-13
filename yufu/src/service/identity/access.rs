use super::{
    account::read_current_actor,
    application::finish_read,
    repository::{AdmissionQuery, IdentityRepository},
};
use crate::{
    domain::identity::{
        account::SecretDigest,
        application::{IdentityScopeSet, LaunchUrl},
        error::IdentityError,
    },
    service::security::repository::now,
};
use auth::security::budget::{Clock, SecurityBudget};
use std::sync::Arc;
use uuid::Uuid;

pub struct AccessService {
    repository: Arc<dyn IdentityRepository>,
    clock: Arc<dyn Clock>,
}

impl AccessService {
    pub fn new(repository: Arc<dyn IdentityRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn launch(
        &self,
        secret: &str,
        environment_id: Uuid,
        budget: &SecurityBudget,
    ) -> Result<LaunchUrl, IdentityError> {
        super::sso::validate_opaque(secret)?;
        let mut tx = self.repository.begin(budget).await?;
        let result = async {
            let actor = read_current_actor(
                tx.as_mut(),
                &SecretDigest::of(secret),
                now(self.clock.as_ref())?,
            )
            .await?;
            let admission = tx
                .evaluate_initial_admission(&AdmissionQuery {
                    sso_session_id: actor.sso_session_id(),
                    environment_id,
                    requested_scopes: IdentityScopeSet::parse("openid")?,
                })
                .await?;
            tx.application_launch(&admission).await
        }
        .await;
        finish_read(tx, result, budget).await
    }
}
