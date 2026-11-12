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
        let member_profiles = self
            .identity_rows(
                "SELECT m.user_id,u.username,u.nickname FROM identity_group_member m JOIN \"user\" u ON u.id=m.user_id WHERE m.group_id=$1 ORDER BY m.user_id LIMIT 1001",
                vec![id.into()],
            )
            .await?
            .into_iter()
            .map(|row| {
                Ok(GroupMemberSummary {
                    user_id: get(&row, "user_id")?,
                    username: get(&row, "username")?,
                    nickname: get::<Option<String>>(&row, "nickname")?.unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>, IdentityError>>()?;
        let members = member_profiles
            .iter()
            .map(|member| member.user_id)
            .collect::<Vec<_>>();
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
            member_profiles,
            application_ids,
        })
    }
    pub(crate) async fn identity_group_ids(
        &self,
        after: Option<Uuid>,
        limit: u32,
        search: Option<&str>,
    ) -> Result<Vec<Uuid>, IdentityError> {
        let pattern = search.map(str::to_owned);
        let exact_id = search.and_then(|value| value.parse::<Uuid>().ok());
        self.identity_rows("SELECT id FROM identity_user_group g WHERE ($1::uuid IS NULL OR id>$1) AND ($3::text IS NULL OR g.id=$4 OR POSITION(lower($3) IN lower(g.name))>0) ORDER BY id LIMIT $2",vec![after.into(),i64::from(limit).into(),pattern.into(),exact_id.into()]).await?
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
    pub(crate) async fn identity_assignment_target_summaries(
        &self,
        app: i32,
    ) -> Result<Vec<AssignmentTargetSummary>, IdentityError> {
        let rows = self
            .identity_rows(
                "SELECT a.target_kind,a.user_id,a.group_id,u.username,u.nickname,g.name AS group_name,(SELECT COUNT(*) FROM identity_group_member m WHERE m.group_id=a.group_id) AS member_count FROM identity_app_assignment a LEFT JOIN \"user\" u ON u.id=a.user_id LEFT JOIN identity_user_group g ON g.id=a.group_id WHERE a.application_id=$1 ORDER BY a.target_kind,a.user_id,a.group_id",
                vec![app.into()],
            )
            .await?;
        rows.into_iter()
            .map(|row| {
                let target = match get::<String>(&row, "target_kind")?.as_str() {
                    "user" => AssignmentTarget::User(get(&row, "user_id")?),
                    "group" => AssignmentTarget::Group(get(&row, "group_id")?),
                    _ => return Err(IdentityError::DependencyUnavailable),
                };
                let (label, secondary) = match target {
                    AssignmentTarget::User(user_id) => {
                        let username = get::<Option<String>>(&row, "username")?
                            .unwrap_or_else(|| format!("account-{user_id}"));
                        let nickname = get::<Option<String>>(&row, "nickname")?
                            .filter(|value| !value.is_empty())
                            .unwrap_or_else(|| username.clone());
                        (nickname, format!("账户 {user_id} · @{username}"))
                    }
                    AssignmentTarget::Group(group_id) => {
                        let name = get::<Option<String>>(&row, "group_name")?
                            .unwrap_or_else(|| format!("用户组 {group_id}"));
                        let member_count = get::<i64>(&row, "member_count")?;
                        (name, format!("用户组 {group_id} · {member_count} 位成员"))
                    }
                };
                Ok(AssignmentTargetSummary {
                    target,
                    label,
                    secondary,
                })
            })
            .collect()
    }
}
