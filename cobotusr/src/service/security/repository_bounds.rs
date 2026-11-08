//! Issuance ceilings use current, structured policy from the same primary snapshot.
use super::*;
use crate::service::permission::{registered_operation, resource_selector};

impl PgSecurityTransaction {
    pub(super) async fn current_application_bounds(
        &self,
        claims: &SecurityAccessClaims,
        application_key: &str,
    ) -> Result<CredentialBounds, SecurityError> {
        validate_application(application_key)?;
        let Some((user, role)) = self.policy_user(claims).await? else {
            return Err(SecurityError::InvalidCredential);
        };
        let Some(app) = self
            .run(
                application::Entity::find()
                    .filter(application::Column::AppKey.eq(application_key))
                    .one(&self.transaction),
            )
            .await?
        else {
            return Ok(CredentialBounds::new(vec![]));
        };
        let rows = self
            .run(
                permission::Entity::find()
                    .filter(permission::Column::ApplicationId.eq(app.id))
                    .all(&self.transaction),
            )
            .await?;
        let overrides = self
            .run(
                user_permission::Entity::find()
                    .filter(user_permission::Column::UserId.eq(user.id))
                    .all(&self.transaction),
            )
            .await?;
        let allowed = self
            .run(
                role_permission::Entity::find()
                    .filter(role_permission::Column::RoleId.eq(user.role_id))
                    .all(&self.transaction),
            )
            .await?;
        let superadmin = role
            .as_ref()
            .is_some_and(|r| r.name == role::RoleEnum::Superadmin.get_name());
        let mut denies = Vec::new();
        for row in &rows {
            if overrides
                .iter()
                .any(|r| r.permission_id == row.id && !r.allow)
            {
                // An ambiguous denied operation cannot be safely subtracted
                // from any issuance ceiling. Fail closed for this application.
                let Some(operation) = registered_operation(row) else {
                    return Ok(CredentialBounds::new(vec![]));
                };
                denies.push((operation, resource_selector(row, application_key).ok()));
            }
        }
        let mut grants = Vec::new();
        for row in &rows {
            if !(overrides
                .iter()
                .any(|r| r.permission_id == row.id && r.allow)
                || (role.is_some()
                    && (superadmin || allowed.iter().any(|r| r.permission_id == row.id))))
            {
                continue;
            }
            let Some(operation) = registered_operation(row) else {
                continue;
            };
            let Ok(selector) = resource_selector(row, application_key) else {
                continue;
            };
            // An allow-minus-deny scope is not representable by the v1 ceiling.
            // Drop every overlapping allow. Removing a deny later must never
            // expand credentials that were issued while that deny was active.
            if denies.iter().any(|(op, deny)| {
                op == &operation
                    && deny
                        .as_ref()
                        .is_none_or(|d| d.intersection(&selector).is_some())
            }) {
                continue;
            }
            grants.push(ScopedOperation::new(operation, vec![selector])?);
        }
        Ok(CredentialBounds::new(grants))
    }
}
