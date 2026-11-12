//! Discover the complete mutation scope before the first lock is acquired.
use super::repository::pg::get;
use super::repository::*;
use crate::domain::identity::error::IdentityError;
use crate::domain::identity::session::IdentityRevocationScope;
use crate::service::security::repository::PgSecurityTransaction;
use std::collections::BTreeSet;
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn plan_identity_revocation(
        &self,
        scope: &IdentityRevocationScope,
    ) -> Result<IdentityRevocationPlan, IdentityError> {
        let (root, rp, code, session, values): (&str, &str, &str, &str, Vec<sea_orm::Value>) =
            match scope {
                IdentityRevocationScope::CurrentCenter { sso_session_id } => (
                    "r.id=$1",
                    "p.sso_session_id=$1",
                    "c.sso_session_id=$1",
                    "s.sso_session_id=$1",
                    vec![(*sso_session_id).into()],
                ),
                IdentityRevocationScope::UserAll { user_id } => (
                    "r.user_id=$1",
                    "p.subject=$1::text",
                    "c.user_id=$1",
                    "s.principal_kind='user' AND s.subject=$1::text",
                    vec![(*user_id).into()],
                ),
                IdentityRevocationScope::Environment { environment_id } => (
                    "r.id IN (SELECT sso_session_id FROM identity_oidc_rp_session WHERE environment_id=$1 UNION SELECT sso_session_id FROM oauth_authorization_code WHERE environment_id=$1)",
                    "p.environment_id=$1",
                    "c.environment_id=$1",
                    "s.environment_id=$1",
                    vec![(*environment_id).into()],
                ),
                IdentityRevocationScope::EnvironmentUser {
                    environment_id,
                    user_id,
                } => (
                    "r.user_id=$2 AND r.id IN (SELECT sso_session_id FROM identity_oidc_rp_session WHERE environment_id=$1 UNION SELECT sso_session_id FROM oauth_authorization_code WHERE environment_id=$1)",
                    "p.environment_id=$1 AND p.subject=$2::text",
                    "c.environment_id=$1 AND c.user_id=$2",
                    "s.environment_id=$1 AND s.subject=$2::text",
                    vec![(*environment_id).into(), (*user_id).into()],
                ),
                IdentityRevocationScope::ApplicationLocal { rp_session_id } => (
                    "r.id IN (SELECT sso_session_id FROM identity_oidc_rp_session WHERE id=$1)",
                    "p.id=$1",
                    "FALSE AND $1::uuid IS NOT NULL",
                    "s.oidc_rp_session_id=$1",
                    vec![(*rp_session_id).into()],
                ),
                IdentityRevocationScope::Grant { grant_id } => (
                    "r.id IN (SELECT s.sso_session_id FROM security_credential c JOIN security_session s ON s.id=c.session_id WHERE c.grant_id=$1)",
                    "p.id IN (SELECT s.oidc_rp_session_id FROM security_credential c JOIN security_session s ON s.id=c.session_id WHERE c.grant_id=$1)",
                    "FALSE AND $1::uuid IS NOT NULL",
                    "s.id IN (SELECT session_id FROM security_credential WHERE grant_id=$1)",
                    vec![(*grant_id).into()],
                ),
            };
        let mut keys = Vec::new();
        let mut user_ids = BTreeSet::new();
        for row in self
            .identity_rows(
                &format!("SELECT r.id,r.user_id FROM identity_sso_session r WHERE {root}"),
                values.clone(),
            )
            .await?
        {
            let user_id: i32 = get(&row, "user_id")?;
            user_ids.insert(user_id);
            keys.extend([
                IdentityLockKey::User(user_id),
                IdentityLockKey::Center(get(&row, "id")?),
            ]);
        }
        let mut rp_session_ids = Vec::new();
        for row in self.identity_rows(&format!("SELECT p.id,p.environment_id,e.application_id,p.subject FROM identity_oidc_rp_session p JOIN identity_app_environment e ON e.id=p.environment_id WHERE {rp}"), values.clone()).await? {
            let id = get(&row, "id")?;
            rp_session_ids.push(id);
            let user_id = get::<String>(&row, "subject")?.parse().map_err(|_| IdentityError::DependencyUnavailable)?;
            user_ids.insert(user_id);
            keys.extend([IdentityLockKey::Application(get(&row, "application_id")?), IdentityLockKey::Environment(get(&row, "environment_id")?), IdentityLockKey::User(user_id), IdentityLockKey::RelyingParty(id)]);
        }
        for row in self.identity_rows(&format!("SELECT c.id,c.flow_id,c.environment_id,e.application_id,c.user_id FROM oauth_authorization_code c JOIN identity_app_environment e ON e.id=c.environment_id WHERE {code} AND c.code_digest IS NOT NULL"), values.clone()).await? {
            let user_id = get(&row, "user_id")?;
            user_ids.insert(user_id);
            keys.extend([IdentityLockKey::Application(get(&row, "application_id")?), IdentityLockKey::Environment(get(&row, "environment_id")?), IdentityLockKey::User(user_id), IdentityLockKey::Code(get(&row, "id")?)]);
            if let Some(flow_id) = get::<Option<Uuid>>(&row, "flow_id")? {
                keys.push(IdentityLockKey::Flow(flow_id));
            }
        }
        for row in self.identity_rows(&format!("SELECT f.id FROM identity_authorization_flow f JOIN identity_sso_session r ON r.id=f.bound_sso_session_id WHERE {root}"), values.clone()).await? {
            keys.push(IdentityLockKey::Flow(get(&row, "id")?));
        }
        // Independent MCP grants have no security_session. They participate in
        // account mutation locking without becoming descendants of a center.
        let direct = match scope {
            IdentityRevocationScope::UserAll { .. } => {
                "c.principal_kind='user' AND c.subject=$1::text"
            }
            IdentityRevocationScope::Grant { .. } => "c.grant_id=$1",
            _ => session,
        };
        let tree = format!(
            "WITH RECURSIVE selected(issuer,jti) AS (SELECT c.issuer,c.jti FROM security_credential c LEFT JOIN security_session s ON s.id=c.session_id WHERE {direct} UNION SELECT c.issuer,c.jti FROM security_credential c JOIN selected parent ON c.issuer=parent.issuer AND c.revocation_parent_jti=parent.jti) SELECT c.issuer,c.jti,c.grant_id,c.session_id,c.family_id,s.sso_session_id,s.oidc_rp_session_id,s.environment_id,e.application_id,c.principal_kind,c.subject FROM selected x JOIN security_credential c ON c.issuer=x.issuer AND c.jti=x.jti LEFT JOIN security_session s ON s.id=c.session_id LEFT JOIN identity_app_environment e ON e.id=s.environment_id"
        );
        for row in self.identity_rows(&tree, values).await? {
            keys.push(IdentityLockKey::Credential {
                issuer: get(&row, "issuer")?,
                jti: get(&row, "jti")?,
            });
            if let Some(id) = get::<Option<i32>>(&row, "application_id")? {
                keys.push(IdentityLockKey::Application(id));
            }
            for (column, key) in [
                (
                    "environment_id",
                    IdentityLockKey::Environment as fn(Uuid) -> IdentityLockKey,
                ),
                ("sso_session_id", IdentityLockKey::Center),
                ("oidc_rp_session_id", IdentityLockKey::RelyingParty),
                ("grant_id", IdentityLockKey::Grant),
                ("session_id", IdentityLockKey::SecuritySession),
                ("family_id", IdentityLockKey::Family),
            ] {
                if let Some(id) = get::<Option<Uuid>>(&row, column)? {
                    keys.push(key(id));
                }
            }
            if get::<Option<String>>(&row, "principal_kind")?.as_deref() == Some("user") {
                let user_id = get::<String>(&row, "subject")?
                    .parse()
                    .map_err(|_| IdentityError::DependencyUnavailable)?;
                user_ids.insert(user_id);
                keys.push(IdentityLockKey::User(user_id));
            }
        }
        match scope {
            IdentityRevocationScope::UserAll { user_id }
            | IdentityRevocationScope::EnvironmentUser { user_id, .. } => {
                keys.push(IdentityLockKey::User(*user_id));
                user_ids.insert(*user_id);
            }
            _ => {}
        }
        if let IdentityRevocationScope::UserAll { user_id } = scope {
            for row in self
                .identity_rows(
                    "SELECT id FROM identity_password_reset WHERE user_id=$1",
                    vec![(*user_id).into()],
                )
                .await?
            {
                keys.push(IdentityLockKey::PasswordReset(get(&row, "id")?));
            }
            for row in self.identity_rows("SELECT f.id FROM identity_action_flow f JOIN identity_password_reset r ON r.id=f.password_reset_id WHERE r.user_id=$1", vec![(*user_id).into()]).await? {
                keys.push(IdentityLockKey::Action(get(&row, "id")?));
            }
        }
        let apps: BTreeSet<i32> = keys
            .iter()
            .filter_map(|key| match key {
                IdentityLockKey::Application(id) => Some(*id),
                _ => None,
            })
            .collect();
        for application_id in apps {
            for row in self.identity_rows("SELECT group_id FROM identity_app_assignment WHERE application_id=$1 AND group_id IS NOT NULL", vec![application_id.into()]).await? {
                keys.push(IdentityLockKey::Group(get(&row, "group_id")?));
            }
            for row in self.identity_rows("SELECT user_id FROM identity_app_member WHERE application_id=$1 AND member_role='owner'", vec![application_id.into()]).await? {
                keys.push(IdentityLockKey::User(get(&row, "user_id")?));
            }
        }
        rp_session_ids.sort();
        rp_session_ids.dedup();
        Ok(IdentityRevocationPlan {
            locks: IdentityLockScope::new(keys)?,
            user_ids,
            rp_session_ids,
        })
    }
}
