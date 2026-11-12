//! Control-panel reads select public management metadata, never credential hashes.
use super::repository::pg::{environment_kind, get, management_mode, unsigned};
use super::repository::*;
use crate::{
    domain::identity::{application::EnvironmentConfig, error::IdentityError},
    service::security::repository::PgSecurityTransaction,
};
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn identity_approval_workspace(
        &self,
        id: Uuid,
    ) -> Result<ApprovalWorkspace, IdentityError> {
        let row=self.identity_one("SELECT p.*,e.application_id,e.kind,v.payload FROM identity_app_approval p JOIN identity_app_environment e ON e.id=p.environment_id JOIN identity_app_config_version v ON v.id=p.version_id AND v.environment_id=e.id WHERE p.id=$1",vec![id.into()]).await?;
        Ok(ApprovalWorkspace {
            approval_id: id,
            application_id: get(&row, "application_id")?,
            environment_id: get(&row, "environment_id")?,
            version_id: get(&row, "version_id")?,
            config: EnvironmentConfig::from_payload(
                get(&row, "payload")?,
                environment_kind(&get::<String>(&row, "kind")?)?,
                true,
            )?
            .to_input(),
            snapshot: serde_json::from_value(get(&row, "snapshot")?)
                .map_err(|_| IdentityError::DependencyUnavailable)?,
            snapshot_hash: get(&row, "snapshot_hash")?,
            requested_by: get(&row, "requested_by")?,
            status: get(&row, "status")?,
            reviewed_by: get(&row, "reviewed_by")?,
            reason: get(&row, "reason")?,
            logout_limitation_accepted: get(&row, "logout_limitation_accepted")?,
        })
    }
    pub(crate) async fn identity_collaboration_visible(
        &self,
        requester: i32,
        candidate: i32,
    ) -> Result<bool, IdentityError> {
        let row=self.identity_one("SELECT EXISTS (SELECT 1 FROM \"user\" u WHERE u.id=$2 AND (u.id=$1 OR EXISTS (SELECT 1 FROM identity_collaboration_candidate c WHERE c.requester_user_id=$1 AND c.candidate_user_id=u.id) OR EXISTS (SELECT 1 FROM identity_app_member m JOIN identity_app_member own ON own.application_id=m.application_id AND own.user_id=$1 WHERE m.user_id=u.id))) AS visible",vec![requester.into(),candidate.into()]).await?;
        get(&row, "visible")
    }
    pub(crate) async fn identity_collaboration_candidates(
        &self,
        requester: i32,
        after: i32,
        limit: u32,
        search: Option<&str>,
    ) -> Result<Vec<CollaborationCandidate>, IdentityError> {
        let pattern = search.map(str::to_owned);
        let exact_id = search.and_then(|value| value.parse::<i32>().ok());
        self.identity_rows("SELECT u.id,u.username,u.nickname FROM \"user\" u WHERE u.id>$2 AND u.enabled AND u.developer_enabled AND (u.id=$1 OR EXISTS (SELECT 1 FROM identity_collaboration_candidate c WHERE c.requester_user_id=$1 AND c.candidate_user_id=u.id) OR EXISTS (SELECT 1 FROM identity_app_member m JOIN identity_app_member own ON own.application_id=m.application_id AND own.user_id=$1 WHERE m.user_id=u.id)) AND ($4::text IS NULL OR u.id=$5 OR POSITION(lower($4) IN lower(u.username))>0 OR POSITION(lower($4) IN lower(COALESCE(u.nickname,'')))>0) ORDER BY u.id LIMIT $3",vec![requester.into(),after.into(),i64::from(limit).into(),pattern.into(),exact_id.into()]).await?.into_iter().map(|row|Ok(CollaborationCandidate { user_id:get(&row,"id")?,username:get(&row,"username")?,nickname:get(&row,"nickname")? })).collect()
    }
    pub(crate) async fn identity_application_workspace(
        &self,
        application_id: i32,
    ) -> Result<ApplicationWorkspace, IdentityError> {
        let app = self.identity_one("SELECT id,app_key,name,description,management_mode,application_kind,lifecycle,identity_revision FROM application WHERE id=$1 AND management_state='managed'", vec![application_id.into()]).await?;
        let members = self.identity_rows("SELECT m.user_id,m.member_role,u.username,u.nickname FROM identity_app_member m JOIN \"user\" u ON u.id=m.user_id WHERE m.application_id=$1 ORDER BY m.user_id", vec![application_id.into()]).await?;
        let mut keys = vec![IdentityLockKey::Application(application_id)];
        let mut summaries = Vec::new();
        let mut owner_id = None;
        for member in members {
            let user_id = get(&member, "user_id")?;
            let member_role: String = get(&member, "member_role")?;
            keys.push(IdentityLockKey::User(user_id));
            if member_role == "owner" && owner_id.replace(user_id).is_some() {
                return Err(IdentityError::DependencyUnavailable);
            }
            summaries.push(ApplicationMemberSummary {
                user_id,
                member_role,
                username: get(&member, "username")?,
                nickname: get::<Option<String>>(&member, "nickname")?.unwrap_or_default(),
            });
        }
        for group in self.identity_rows("SELECT group_id FROM identity_app_assignment WHERE application_id=$1 AND group_id IS NOT NULL ORDER BY group_id", vec![application_id.into()]).await? {
            keys.push(IdentityLockKey::Group(get(&group, "group_id")?));
        }
        let rows = self.identity_rows("SELECT e.id,e.kind,e.revision,e.lifecycle_epoch,e.active_version_id,c.client_id,c.public_client,c.secret_revision,v.id AS version_id,v.version,v.payload,vc.client_id AS candidate_client_id,av.payload AS active_payload,(SELECT p.id FROM identity_app_approval p WHERE p.environment_id=e.id AND p.status='pending' ORDER BY p.created_at DESC LIMIT 1) AS pending_approval FROM identity_app_environment e JOIN oauth_client c ON c.id=e.oauth_client_id AND c.environment_id=e.id JOIN LATERAL (SELECT * FROM identity_app_config_version v WHERE v.environment_id=e.id ORDER BY v.version DESC LIMIT 1) v ON TRUE LEFT JOIN oauth_client vc ON vc.id=v.candidate_oauth_client_id LEFT JOIN identity_app_config_version av ON av.id=e.active_version_id AND av.environment_id=e.id WHERE e.application_id=$1 ORDER BY e.kind", vec![application_id.into()]).await?;
        let mut environments = Vec::new();
        for row in rows {
            let environment_id = get(&row, "id")?;
            keys.push(IdentityLockKey::Environment(environment_id));
            let kind = environment_kind(&get::<String>(&row, "kind")?)?;
            let config =
                EnvironmentConfig::from_payload(get(&row, "payload")?, kind, false)?.to_input();
            let active_payload: Option<serde_json::Value> = get(&row, "active_payload")?;
            let active_config = active_payload
                .map(|payload| {
                    EnvironmentConfig::from_payload(payload, kind, true).map(|c| c.to_input())
                })
                .transpose()?;
            environments.push(EnvironmentWorkspace {
                environment_id,
                kind,
                client_id: get(&row, "client_id")?,
                primary_client_kind: if get::<bool>(&row, "public_client")? {
                    crate::domain::identity::application::ClientKind::BrowserSpa
                } else {
                    crate::domain::identity::application::ClientKind::ServerWeb
                },
                secret_revision: unsigned(get(&row, "secret_revision")?)?,
                revision: unsigned(get(&row, "revision")?)?,
                lifecycle_epoch: unsigned(get(&row, "lifecycle_epoch")?)?,
                active_version_id: get(&row, "active_version_id")?,
                active_config,
                pending_approval: get(&row, "pending_approval")?,
                latest: ConfigVersionSummary {
                    version_id: get(&row, "version_id")?,
                    version: unsigned(get(&row, "version")?)?,
                    config,
                    candidate_client_id: get(&row, "candidate_client_id")?,
                },
            });
        }
        if environments.len() != 2 {
            return Err(IdentityError::DependencyUnavailable);
        }
        Ok(ApplicationWorkspace {
            application_id,
            app_key: get(&app, "app_key")?,
            name: get(&app, "name")?,
            description: get(&app, "description")?,
            management_mode: management_mode(&get::<String>(&app, "management_mode")?)?,
            third_party: get::<String>(&app, "application_kind")? == "third_party",
            active: get::<String>(&app, "lifecycle")? == "active",
            revision: unsigned(get(&app, "identity_revision")?)?,
            owner_user_id: owner_id.ok_or(IdentityError::OwnerRequired)?,
            members: summaries,
            environments,
            locks: IdentityLockScope::new(keys)?,
        })
    }
    pub(crate) async fn identity_developer_application_ids(
        &self,
        user_id: i32,
        after: i32,
        limit: u32,
    ) -> Result<Vec<i32>, IdentityError> {
        self.identity_rows("SELECT a.id FROM application a JOIN identity_app_member m ON m.application_id=a.id WHERE a.management_state='managed' AND m.user_id=$1 AND a.id>$2 ORDER BY a.id LIMIT $3", vec![user_id.into(),after.into(),i64::from(limit).into()]).await?
            .into_iter().map(|row| get(&row, "id")).collect()
    }
    pub(crate) async fn identity_reader_summaries(
        &self,
        environment_id: Uuid,
        after: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<ReaderSummary>, IdentityError> {
        self.identity_rows("SELECT id,reader_id,environment_id,approved_audience,enabled,secret_revision,created_at FROM identity_introspection_reader WHERE environment_id=$1 AND ($2::uuid IS NULL OR id>$2) ORDER BY id LIMIT $3", vec![environment_id.into(),after.into(),i64::from(limit).into()]).await?
            .into_iter().map(|row| Ok(ReaderSummary { id: get(&row,"id")?, reader_id: get(&row,"reader_id")?, environment_id: get(&row,"environment_id")?,
                approved_audience: get(&row,"approved_audience")?, enabled: get(&row,"enabled")?, revision: unsigned(get(&row,"secret_revision")?)?, created_at: unsigned(get(&row,"created_at")?)? })).collect()
    }
    pub(crate) async fn identity_operation_receipt_by_id(
        &self,
        actor_id: i32,
        operation_id: Uuid,
    ) -> Result<Option<SafeOperationReceipt>, IdentityError> {
        let rows = self.identity_rows("SELECT id,request_hash,resource_id,result_summary,committed_at FROM identity_request_operation WHERE id=$1 AND actor_id=$2", vec![operation_id.into(),actor_id.into()]).await?;
        rows.first()
            .map(|row| {
                Ok(SafeOperationReceipt {
                    operation_id: get(row, "id")?,
                    request_hash: get(row, "request_hash")?,
                    resource_id: get(row, "resource_id")?,
                    summary: get(row, "result_summary")?,
                    committed_at: unsigned(get(row, "committed_at")?)?,
                })
            })
            .transpose()
    }
}
