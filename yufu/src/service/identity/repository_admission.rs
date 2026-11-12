use super::repository::pg::{environment_kind, get, management_mode, one_row, signed};
use super::repository::*;
use crate::domain::identity::{error::IdentityError, session::IdentityRevocationScope};
use crate::service::security::repository::{PgSecurityTransaction, now};
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn write_identity_admission(
        &mut self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        match command {
            IdentityWrite::EnsureAccessGeneration {
                environment_id,
                user_id,
            } => {
                self.ensure_identity_generation(
                    environment_id,
                    user_id,
                    super::repository::pg::unsigned(at)?,
                )
                .await
            }
            IdentityWrite::SetAssignment {
                application_id,
                target,
                present,
                actor_id,
            } => {
                one_row(self.identity_execute("UPDATE application SET identity_revision=identity_revision+1,access_revision=access_revision+1 WHERE id=$1 AND management_state='managed'",vec![application_id.into()]).await?)?;
                let (kind, user, group) = match target {
                    AssignmentTarget::User(id) => ("user", Some(id), None),
                    AssignmentTarget::Group(id) => ("group", None, Some(id)),
                };
                if present {
                    self.identity_execute("INSERT INTO identity_app_assignment (id,application_id,target_kind,user_id,group_id,created_by,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",vec![Uuid::new_v4().into(),application_id.into(),kind.into(),user.into(),group.into(),actor_id.into(),at.into()]).await?;
                } else {
                    self.identity_execute("DELETE FROM identity_app_assignment WHERE application_id=$1 AND target_kind=$2 AND user_id IS NOT DISTINCT FROM $3 AND group_id IS NOT DISTINCT FROM $4",vec![application_id.into(),kind.into(),user.into(),group.into()]).await?;
                }
                self.obsolete_identity_approvals(application_id, at).await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::SetTestUser {
                environment_id,
                user_id,
                present,
                actor_id,
            } => {
                let env = self.identity_one("SELECT application_id FROM identity_app_environment WHERE id=$1 AND kind='development'",vec![environment_id.into()]).await?;
                one_row(self.identity_execute("UPDATE application SET identity_revision=identity_revision+1,access_revision=access_revision+1 WHERE id=$1",vec![get::<i32>(&env,"application_id")?.into()]).await?)?;
                if present {
                    self.identity_execute("INSERT INTO identity_app_test_user (environment_id,user_id,created_by,created_at) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING",vec![environment_id.into(),user_id.into(),actor_id.into(),at.into()]).await?;
                } else {
                    self.identity_execute(
                        "DELETE FROM identity_app_test_user WHERE environment_id=$1 AND user_id=$2",
                        vec![environment_id.into(), user_id.into()],
                    )
                    .await?;
                }
                self.obsolete_identity_approvals(get(&env, "application_id")?, at)
                    .await?;
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::RecomputeGeneration {
                environment_id,
                user_id,
            } => {
                let account = self
                    .identity_account(user_id)
                    .await?
                    .ok_or(IdentityError::NotFound)?;
                let env = self.identity_one("SELECT e.kind,e.application_id,a.management_mode FROM identity_app_environment e JOIN application a ON a.id=e.application_id WHERE e.id=$1",vec![environment_id.into()]).await?;
                let sources = self
                    .identity_sources(
                        get(&env, "application_id")?,
                        environment_id,
                        &account,
                        management_mode(&get::<String>(&env, "management_mode")?)?,
                    )
                    .await?;
                let eligible = account.enabled
                    && sources.eligible(environment_kind(&get::<String>(&env, "kind")?)?);
                // Eligibility records source membership. Suspension is an epoch
                // change and must not reset or revive a membership generation.
                let row = self.identity_one("INSERT INTO identity_access_generation (environment_id,user_id,generation,eligible,changed_at) VALUES ($1,$2,0,$3,$4) ON CONFLICT (environment_id,user_id) DO UPDATE SET generation=identity_access_generation.generation+CASE WHEN identity_access_generation.eligible AND NOT EXCLUDED.eligible THEN 1 ELSE 0 END,eligible=EXCLUDED.eligible,changed_at=CASE WHEN identity_access_generation.eligible<>EXCLUDED.eligible THEN EXCLUDED.changed_at ELSE identity_access_generation.changed_at END RETURNING generation",vec![environment_id.into(),user_id.into(),eligible.into(),at.into()]).await?;
                if !eligible {
                    self.revoke_identity_scope(
                        &IdentityRevocationScope::EnvironmentUser {
                            environment_id,
                            user_id,
                        },
                        at,
                    )
                    .await?;
                }
                Ok(IdentityWriteResult::Revision(
                    super::repository::pg::unsigned(get(&row, "generation")?)?,
                ))
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }
}
