//! Discover the entire mutation, acquire the common locks, then reload every dependency.
use super::*;

pub(super) struct AssignmentPlan {
    pub(super) locks: IdentityLockScope,
    pub(super) pairs: Vec<(Uuid, i32)>,
    pub(super) rp_ids: Vec<Uuid>,
    group: Option<GroupWorkspace>,
    apps: Vec<ApplicationWorkspace>,
    revocations: Vec<(IdentityRevocationScope, IdentityRevocationPlan)>,
}
impl AssignmentPlan {
    pub(super) fn application_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.apps.iter().map(|app| app.application_id)
    }
    pub(super) async fn load(
        tx: &mut dyn IdentityTransaction,
        mutation: &AssignmentMutation,
    ) -> Result<Self, IdentityError> {
        let mut users = Vec::new();
        let mut app_ids = Vec::new();
        let group_id = match mutation {
            AssignmentMutation::CreateGroup { .. } => None,
            AssignmentMutation::UpdateGroup { group_id, .. } => Some(*group_id),
            AssignmentMutation::SetGroupMember {
                group_id, user_id, ..
            } => {
                users.push(*user_id);
                Some(*group_id)
            }
            AssignmentMutation::DeleteGroup { group_id, .. } => Some(*group_id),
            AssignmentMutation::SetAssignment {
                application_id,
                target,
                ..
            } => {
                app_ids.push(*application_id);
                match target {
                    AssignmentTarget::User(user) => {
                        users.push(*user);
                        None
                    }
                    AssignmentTarget::Group(group) => Some(*group),
                }
            }
            AssignmentMutation::SetTestUser {
                application_id,
                user_id,
                ..
            } => {
                app_ids.push(*application_id);
                users.push(*user_id);
                None
            }
        };
        let group = match group_id {
            Some(id) => Some(tx.group_workspace(id).await?),
            None => None,
        };
        if let Some(group) = &group {
            match mutation {
                AssignmentMutation::SetGroupMember { .. } => {
                    app_ids = group.application_ids.clone()
                }
                AssignmentMutation::DeleteGroup { .. } => {
                    app_ids = group.application_ids.clone();
                    users = group.members.clone();
                }
                AssignmentMutation::SetAssignment { .. } => users = group.members.clone(),
                _ => {}
            }
        }
        users.sort();
        users.dedup();
        app_ids.sort();
        app_ids.dedup();
        if users.len() > 1000 || app_ids.len() > 1000 {
            return Err(IdentityError::CapacityExceeded);
        }
        let mut keys = Vec::new();
        if let Some(id) = group_id {
            keys.push(IdentityLockKey::Group(id));
        }
        keys.extend(users.iter().map(|id| IdentityLockKey::User(*id)));
        let mut apps = Vec::new();
        let mut pairs = Vec::new();
        for id in app_ids {
            let app = tx.application_workspace(id).await?;
            keys.extend_from_slice(app.locks.keys());
            for env in &app.environments {
                if let AssignmentMutation::SetTestUser { environment_id, .. } = mutation
                    && *environment_id != env.environment_id
                {
                    continue;
                }
                pairs.extend(users.iter().map(|user| (env.environment_id, *user)));
            }
            apps.push(app);
        }
        if pairs.len() > 10000 {
            return Err(IdentityError::CapacityExceeded);
        }
        let mut revocations = Vec::new();
        let mut rp_ids = Vec::new();
        for (environment_id, user_id) in &pairs {
            let scope = IdentityRevocationScope::EnvironmentUser {
                environment_id: *environment_id,
                user_id: *user_id,
            };
            let plan = tx.revocation_plan(&scope).await?;
            keys.extend_from_slice(plan.locks.keys());
            rp_ids.extend_from_slice(&plan.rp_session_ids);
            revocations.push((scope, plan));
        }
        rp_ids.sort();
        rp_ids.dedup();
        Ok(Self {
            locks: IdentityLockScope::new(keys)?,
            pairs,
            rp_ids,
            group,
            apps,
            revocations,
        })
    }
    pub(super) async fn recheck(
        &self,
        tx: &mut dyn IdentityTransaction,
        mutation: &AssignmentMutation,
    ) -> Result<(), IdentityError> {
        if let Some(group) = &self.group
            && tx.group_workspace(group.group_id).await? != *group
        {
            return Err(IdentityError::Conflict);
        }
        for app in &self.apps {
            let current = tx.application_workspace(app.application_id).await?;
            if current.revision != app.revision || current.locks.keys() != app.locks.keys() {
                return Err(IdentityError::Conflict);
            }
        }
        for (scope, plan) in &self.revocations {
            let current = tx.revocation_plan(scope).await?;
            if current.locks.keys() != plan.locks.keys()
                || current.rp_session_ids != plan.rp_session_ids
            {
                return Err(IdentityError::Conflict);
            }
        }
        let (revision, expected) = match mutation {
            AssignmentMutation::CreateGroup { .. } => return Ok(()),
            AssignmentMutation::UpdateGroup {
                expected_revision, ..
            }
            | AssignmentMutation::SetGroupMember {
                expected_revision, ..
            }
            | AssignmentMutation::DeleteGroup {
                expected_revision, ..
            } => (
                self.group.as_ref().ok_or(IdentityError::NotFound)?.revision,
                *expected_revision,
            ),
            AssignmentMutation::SetAssignment {
                expected_revision, ..
            }
            | AssignmentMutation::SetTestUser {
                expected_revision, ..
            } => (
                self.apps.first().ok_or(IdentityError::NotFound)?.revision,
                *expected_revision,
            ),
        };
        if revision != expected {
            return Err(IdentityError::Conflict);
        }
        Ok(())
    }
    pub(super) async fn apply(
        &self,
        tx: &mut dyn IdentityTransaction,
        mutation: &AssignmentMutation,
        actor: i32,
        new_group: Uuid,
    ) -> Result<(serde_json::Value, String, bool), IdentityError> {
        let mut summary = serde_json::json!({});
        let (resource, changed) = match mutation {
            AssignmentMutation::CreateGroup { name, description } => {
                tx.apply(IdentityWrite::CreateGroup {
                    id: new_group,
                    name: name.clone(),
                    description: description.clone(),
                    actor_id: actor,
                })
                .await?;
                summary["group_id"] = serde_json::json!(new_group);
                summary["revision"] = serde_json::json!(0);
                summary["created"] = serde_json::json!(true);
                (new_group.to_string(), true)
            }
            AssignmentMutation::UpdateGroup {
                group_id,
                name,
                description,
                expected_revision,
            } => {
                let group = self.group.as_ref().ok_or(IdentityError::NotFound)?;
                let changed = group.name != *name || group.description != *description;
                if changed {
                    tx.apply(IdentityWrite::UpdateGroup {
                        group_id: *group_id,
                        name: name.clone(),
                        description: description.clone(),
                        expected_revision: *expected_revision,
                    })
                    .await?;
                }
                summary["group_id"] = serde_json::json!(group_id);
                summary["metadata_changed"] = serde_json::json!(changed);
                summary["revision"] = serde_json::json!(expected_revision + u64::from(changed));
                (group_id.to_string(), changed)
            }
            AssignmentMutation::SetGroupMember {
                group_id,
                user_id,
                present,
                expected_revision,
            } => {
                if *present {
                    require_enabled(tx, *user_id).await?;
                }
                let group = self.group.as_ref().ok_or(IdentityError::NotFound)?;
                let changed = group.members.contains(user_id) != *present;
                if changed {
                    tx.apply(IdentityWrite::ChangeGroupMember {
                        group_id: *group_id,
                        user_id: *user_id,
                        present: *present,
                        expected_revision: *expected_revision,
                        actor_id: actor,
                    })
                    .await?;
                }
                summary["group_id"] = serde_json::json!(group_id);
                summary["target_user_id"] = serde_json::json!(user_id);
                summary["present"] = serde_json::json!(present);
                summary["revision"] = serde_json::json!(expected_revision + u64::from(changed));
                (group_id.to_string(), changed)
            }
            AssignmentMutation::DeleteGroup {
                group_id,
                expected_revision,
            } => {
                for app in &self.apps {
                    tx.apply(IdentityWrite::SetAssignment {
                        application_id: app.application_id,
                        target: AssignmentTarget::Group(*group_id),
                        present: false,
                        actor_id: actor,
                    })
                    .await?;
                }
                tx.apply(IdentityWrite::DeleteGroup {
                    group_id: *group_id,
                    expected_revision: *expected_revision,
                })
                .await?;
                summary["group_id"] = serde_json::json!(group_id);
                summary["active"] = serde_json::json!(false);
                (group_id.to_string(), true)
            }
            AssignmentMutation::SetAssignment {
                application_id,
                target,
                present,
                expected_revision,
            } => {
                if let AssignmentTarget::User(user) = target
                    && *present
                {
                    require_enabled(tx, *user).await?;
                }
                let changed = tx
                    .assignment_targets(*application_id)
                    .await?
                    .contains(target)
                    != *present;
                if changed {
                    tx.apply(IdentityWrite::SetAssignment {
                        application_id: *application_id,
                        target: target.clone(),
                        present: *present,
                        actor_id: actor,
                    })
                    .await?;
                }
                summary["application_id"] = serde_json::json!(application_id);
                summary["present"] = serde_json::json!(present);
                summary["revision"] = serde_json::json!(expected_revision + u64::from(changed));
                match target {
                    AssignmentTarget::User(user) => {
                        summary["target_user_id"] = serde_json::json!(user)
                    }
                    AssignmentTarget::Group(group) => {
                        summary["group_id"] = serde_json::json!(group)
                    }
                };
                (application_id.to_string(), changed)
            }
            AssignmentMutation::SetTestUser {
                application_id,
                environment_id,
                user_id,
                present,
                expected_revision,
            } => {
                let app = self.apps.first().ok_or(IdentityError::NotFound)?;
                let env = app
                    .environments
                    .iter()
                    .find(|env| env.environment_id == *environment_id)
                    .ok_or(IdentityError::NotFound)?;
                if env.kind != crate::domain::identity::application::AppEnvironmentKind::Development
                {
                    return Err(IdentityError::InvalidInput);
                }
                if *present {
                    require_enabled(tx, *user_id).await?;
                }
                tx.apply(IdentityWrite::SetTestUser {
                    environment_id: *environment_id,
                    user_id: *user_id,
                    present: *present,
                    actor_id: actor,
                })
                .await?;
                summary["application_id"] = serde_json::json!(application_id);
                summary["environment_id"] = serde_json::json!(environment_id);
                summary["target_user_id"] = serde_json::json!(user_id);
                summary["present"] = serde_json::json!(present);
                summary["revision"] = serde_json::json!(expected_revision + 1);
                (application_id.to_string(), true)
            }
        };
        Ok((summary, resource, changed))
    }
}
async fn require_enabled(tx: &mut dyn IdentityTransaction, user: i32) -> Result<(), IdentityError> {
    if tx
        .load_account(user)
        .await?
        .is_none_or(|account| !account.enabled)
    {
        Err(IdentityError::Forbidden)
    } else {
        Ok(())
    }
}
