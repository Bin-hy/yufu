use super::repository::pg::{get, unsigned};
use super::repository::*;
use crate::domain::identity::error::IdentityError;
use crate::service::security::repository::PgSecurityTransaction;
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn identity_group_workspace(
        &self,
        id: Uuid,
    ) -> Result<GroupWorkspace, IdentityError> {
        let row = self
            .identity_one(
                "SELECT id,name,description,revision FROM identity_user_group WHERE id=$1",
                vec![id.into()],
            )
            .await?;
        let members = self
            .identity_rows(
                "SELECT user_id FROM identity_group_member WHERE group_id=$1 ORDER BY user_id LIMIT 1001",
                vec![id.into()],
            )
            .await?
            .iter()
            .map(|row| get(row, "user_id"))
            .collect::<Result<Vec<_>, _>>()?;
        let application_ids = self.identity_rows("SELECT application_id FROM identity_app_assignment WHERE group_id=$1 ORDER BY application_id LIMIT 1001", vec![id.into()]).await?
            .iter().map(|row| get(row, "application_id")).collect::<Result<Vec<_>, _>>()?;
        if members.len() > 1000 || application_ids.len() > 1000 {
            return Err(IdentityError::CapacityExceeded);
        }
        Ok(GroupWorkspace {
            group_id: id,
            name: get(&row, "name")?,
            description: get(&row, "description")?,
            revision: unsigned(get(&row, "revision")?)?,
            members,
            application_ids,
        })
    }
    pub(crate) async fn identity_group_ids(
        &self,
        after: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<Uuid>, IdentityError> {
        self.identity_rows("SELECT id FROM identity_user_group WHERE ($1::uuid IS NULL OR id>$1) ORDER BY id LIMIT $2",vec![after.into(),i64::from(limit).into()]).await?
            .iter().map(|row| get(row,"id")).collect()
    }
    pub(crate) async fn identity_assignment_targets(
        &self,
        app: i32,
    ) -> Result<Vec<AssignmentTarget>, IdentityError> {
        let mut targets = Vec::new();
        for row in self.identity_rows("SELECT target_kind,user_id,group_id FROM identity_app_assignment WHERE application_id=$1 ORDER BY target_kind,user_id,group_id",vec![app.into()]).await? {
            targets.push(match get::<String>(&row,"target_kind")?.as_str() {
                "user" => AssignmentTarget::User(get(&row,"user_id")?),
                "group" => AssignmentTarget::Group(get(&row,"group_id")?),
                _ => return Err(IdentityError::DependencyUnavailable),
            });
        }
        targets.sort();
        Ok(targets)
    }
}
