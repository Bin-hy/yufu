pub use crate::domain::identity::application::AssignmentTarget;
use crate::domain::identity::{
    account::{AccountSnapshot, InvitationPurpose, ManagementMode, PersonKind, SecretDigest},
    admission::ManagedAdmission,
    application::{ApprovalSnapshot, EnvironmentConfig, IdentityScopeSet},
    error::IdentityError,
    session::{BrowserSessionSnapshot, FlowHandle, IdentityRevocationScope},
};
use crate::service::security::repository::SecurityTransaction;
use auth::security::{budget::SecurityBudget, identity::SecretToken};
use uuid::Uuid;

#[path = "repository_pg.rs"]
pub(crate) mod pg;
pub use pg::PgIdentityRepository;

#[async_trait::async_trait]
#[allow(clippy::double_must_use)]
pub trait IdentityRepository: Send + Sync {
    async fn begin(
        &self,
        budget: &SecurityBudget,
    ) -> Result<Box<dyn IdentityTransaction>, IdentityError>;
}

#[async_trait::async_trait]
#[allow(clippy::double_must_use)]
pub trait IdentityTransaction: SecurityTransaction {
    async fn console_read(
        &mut self,
        query: crate::domain::identity::console::ConsoleQuery,
    ) -> Result<crate::domain::identity::console::ConsoleResult, IdentityError>;
    async fn personal_consent_environment(
        &mut self,
        user_id: i32,
        consent_id: i32,
    ) -> Result<Option<(i32, Uuid)>, IdentityError>;
    async fn legacy_client_ids(
        &mut self,
        after: i32,
        limit: u32,
    ) -> Result<Vec<i32>, IdentityError>;
    async fn legacy_workspace(&mut self, client_id: i32) -> Result<LegacyWorkspace, IdentityError>;
    async fn logout_rp_reference(
        &mut self,
        reference: &auth::claims::LogoutReference,
    ) -> Result<Option<crate::entity::identity_oidc_rp_session::Model>, IdentityError>;
    async fn group_workspace(&mut self, group_id: Uuid) -> Result<GroupWorkspace, IdentityError>;
    async fn group_ids(
        &mut self,
        after: Option<Uuid>,
        limit: u32,
        search: Option<&str>,
    ) -> Result<Vec<Uuid>, IdentityError>;
    async fn assignment_targets(
        &mut self,
        application_id: i32,
    ) -> Result<Vec<AssignmentTarget>, IdentityError>;
    async fn assignment_target_summaries(
        &mut self,
        application_id: i32,
    ) -> Result<Vec<AssignmentTargetSummary>, IdentityError>;
    async fn application_workspace(
        &mut self,
        application_id: i32,
    ) -> Result<ApplicationWorkspace, IdentityError>;
    async fn approval_workspace(
        &mut self,
        approval_id: Uuid,
    ) -> Result<ApprovalWorkspace, IdentityError>;
    async fn collaboration_visible(
        &mut self,
        requester_id: i32,
        candidate_id: i32,
    ) -> Result<bool, IdentityError>;
    async fn collaboration_candidates(
        &mut self,
        requester_id: i32,
        after: i32,
        limit: u32,
        search: Option<&str>,
    ) -> Result<Vec<CollaborationCandidate>, IdentityError>;
    async fn administrator_application_ids(
        &mut self,
        after: i32,
        limit: u32,
    ) -> Result<Vec<i32>, IdentityError>;
    async fn live_rp_ids(&mut self, ids: &[Uuid]) -> Result<Vec<Uuid>, IdentityError>;
    async fn developer_application_ids(
        &mut self,
        user_id: i32,
        after: i32,
        limit: u32,
    ) -> Result<Vec<i32>, IdentityError>;
    async fn reader_summaries(
        &mut self,
        environment_id: Uuid,
        after: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<ReaderSummary>, IdentityError>;
    async fn operation_receipt_by_id(
        &mut self,
        actor_id: i32,
        operation_id: Uuid,
    ) -> Result<Option<SafeOperationReceipt>, IdentityError>;
    async fn approved_spa_origin(&mut self, origin: &str) -> Result<bool, IdentityError>;
    async fn introspection_reader(
        &mut self,
        reader_id: &str,
    ) -> Result<Option<crate::entity::identity_introspection_reader::Model>, IdentityError>;
    async fn environment_client(
        &mut self,
        environment_id: Uuid,
    ) -> Result<Option<crate::entity::oauth_client::Model>, IdentityError>;
    async fn application_maintainer(
        &mut self,
        application_id: i32,
        user_id: i32,
    ) -> Result<bool, IdentityError>;
    /// Historical registration lookup for revocation, independent of usability.
    async fn protocol_revocation_target(
        &mut self,
        issuer: &str,
        token: &SecretToken,
        access: Option<&auth::security::credential::CredentialRef>,
    ) -> Result<Option<ProtocolRevocationTarget>, IdentityError>;
    async fn authorization_target(
        &mut self,
        client_id: &str,
        user_id: Option<i32>,
    ) -> Result<AuthorizationTarget, IdentityError>;
    async fn current_consent(
        &mut self,
        user_id: i32,
        environment_id: Uuid,
        version_id: Uuid,
    ) -> Result<Option<IdentityScopeSet>, IdentityError>;
    async fn code_token_context(
        &mut self,
        code: &crate::entity::oauth_authorization_code::Model,
    ) -> Result<crate::domain::identity::session::IdentityTokenContext, IdentityError>;
    async fn account_governance(
        &mut self,
        user_id: i32,
    ) -> Result<AccountGovernanceSnapshot, IdentityError>;
    async fn authorize_identity_operation(
        &mut self,
        actor: &crate::domain::identity::account::VerifiedActor,
        operation: crate::domain::identity::operation::IdentityOperation,
    ) -> Result<(), IdentityError>;
    async fn lock_identity_scope(&mut self, scope: &IdentityLockScope)
    -> Result<(), IdentityError>;
    async fn load_account(
        &mut self,
        user_id: i32,
    ) -> Result<Option<AccountSnapshot>, IdentityError>;
    async fn load_browser_session(
        &mut self,
        digest: &SecretDigest,
    ) -> Result<Option<BrowserSessionSnapshot>, IdentityError>;
    async fn evaluate_admission(
        &mut self,
        query: &AdmissionQuery,
    ) -> Result<ManagedAdmission, IdentityError>;
    /// A first authorization can inspect eligibility before creating its generation row.
    async fn evaluate_initial_admission(
        &mut self,
        query: &AdmissionQuery,
    ) -> Result<ManagedAdmission, IdentityError>;
    async fn approval_snapshot(
        &mut self,
        environment_id: Uuid,
        version_id: Uuid,
        logout_limitation_accepted: bool,
    ) -> Result<ApprovalSnapshot, IdentityError>;
    async fn operation_receipt(
        &mut self,
        actor_id: i32,
        kind: &str,
        key: &str,
    ) -> Result<Option<SafeOperationReceipt>, IdentityError>;
    async fn invitation_by_digest(
        &mut self,
        digest: &SecretDigest,
    ) -> Result<Option<crate::domain::identity::account::InvitationSnapshot>, IdentityError>;
    async fn reset_by_digest(
        &mut self,
        digest: &SecretDigest,
    ) -> Result<Option<crate::domain::identity::account::PasswordResetSnapshot>, IdentityError>;
    async fn load_invitation(
        &mut self,
        id: Uuid,
    ) -> Result<Option<crate::domain::identity::account::InvitationSnapshot>, IdentityError>;
    async fn load_reset(
        &mut self,
        id: Uuid,
    ) -> Result<Option<crate::domain::identity::account::PasswordResetSnapshot>, IdentityError>;
    async fn load_action_flow(
        &mut self,
        id: Uuid,
    ) -> Result<Option<crate::domain::identity::session::ActionFlowSnapshot>, IdentityError>;
    async fn load_profile(
        &mut self,
        user_id: i32,
    ) -> Result<Option<auth::claims::IdentityProfile>, IdentityError>;
    async fn load_password_account(
        &mut self,
        username: &str,
    ) -> Result<Option<PasswordAccount>, IdentityError>;
    async fn revocation_plan(
        &mut self,
        scope: &IdentityRevocationScope,
    ) -> Result<IdentityRevocationPlan, IdentityError>;
    async fn load_authorization_flow(
        &mut self,
        id: Uuid,
    ) -> Result<Option<crate::domain::identity::session::AuthorizationFlowSnapshot>, IdentityError>;
    async fn apply(&mut self, command: IdentityWrite)
    -> Result<IdentityWriteResult, IdentityError>;
    async fn append_audit(&mut self, event: &AuditEvent) -> Result<(), IdentityError>;
    async fn enqueue_logout(&mut self, event: &LogoutEvent) -> Result<(), IdentityError>;
    // commit and rollback are deliberately inherited from SecurityTransaction.
}

/// The declaration order is the global acquisition order. It is independent
/// of the order in which a caller discovered resources.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityLockKey {
    Policy,
    Application(i32),
    Environment(Uuid),
    OAuthClient(i32),
    Reader(Uuid),
    Group(Uuid),
    User(i32),
    Center(Uuid),
    Flow(Uuid),
    Action(Uuid),
    Invitation(Uuid),
    PasswordReset(Uuid),
    Code(i32),
    RelyingParty(Uuid),
    Grant(Uuid),
    SecuritySession(Uuid),
    Family(Uuid),
    Credential { issuer: String, jti: String },
}

#[derive(Debug, Clone)]
pub struct IdentityLockScope {
    keys: Vec<IdentityLockKey>,
}
impl IdentityLockScope {
    pub fn new(mut keys: Vec<IdentityLockKey>) -> Result<Self, IdentityError> {
        if keys.len() > 10_000
            || keys.iter().any(|key| match key {
                IdentityLockKey::Application(id)
                | IdentityLockKey::OAuthClient(id)
                | IdentityLockKey::User(id)
                | IdentityLockKey::Code(id) => *id <= 0,
                IdentityLockKey::Credential { issuer, jti } => issuer.is_empty() || jti.is_empty(),
                _ => false,
            })
        {
            return Err(IdentityError::InvalidInput);
        }
        keys.sort();
        keys.dedup();
        Ok(Self { keys })
    }
    pub fn keys(&self) -> &[IdentityLockKey] {
        &self.keys
    }
}

/// IDs and requested scopes only. Current facts are loaded from the shared primary snapshot.
pub struct AdmissionQuery {
    pub sso_session_id: Uuid,
    pub environment_id: Uuid,
    pub requested_scopes: IdentityScopeSet,
}

pub struct AuthorizationTarget {
    pub application_id: i32,
    pub application_name: String,
    pub environment_id: Uuid,
    pub config_version_id: Uuid,
    pub config: EnvironmentConfig,
    pub enabled: bool,
    pub locks: IdentityLockScope,
}

#[derive(serde::Serialize)]
pub struct ApplicationWorkspace {
    pub application_id: i32,
    pub app_key: String,
    pub name: String,
    pub description: Option<String>,
    pub management_mode: ManagementMode,
    pub third_party: bool,
    pub active: bool,
    pub revision: u64,
    pub owner_user_id: i32,
    pub members: Vec<ApplicationMemberSummary>,
    pub environments: Vec<EnvironmentWorkspace>,
    #[serde(skip_serializing)]
    pub locks: IdentityLockScope,
}
#[derive(serde::Serialize)]
pub struct ApplicationMemberSummary {
    pub user_id: i32,
    pub member_role: String,
    pub username: String,
    pub nickname: String,
}
#[derive(serde::Serialize)]
pub struct EnvironmentWorkspace {
    pub environment_id: Uuid,
    pub kind: crate::domain::identity::application::AppEnvironmentKind,
    pub client_id: String,
    pub primary_client_kind: crate::domain::identity::application::ClientKind,
    pub secret_revision: u64,
    pub revision: u64,
    pub lifecycle_epoch: u64,
    pub active_version_id: Option<Uuid>,
    pub latest: ConfigVersionSummary,
    pub active_config: Option<crate::domain::identity::application::EnvironmentConfigInput>,
    pub pending_approval: Option<Uuid>,
}
#[derive(serde::Serialize)]
pub struct ApprovalWorkspace {
    pub approval_id: Uuid,
    pub application_id: i32,
    pub environment_id: Uuid,
    pub version_id: Uuid,
    pub config: crate::domain::identity::application::EnvironmentConfigInput,
    pub snapshot: ApprovalSnapshot,
    pub snapshot_hash: String,
    pub requested_by: i32,
    pub status: String,
    pub reviewed_by: Option<i32>,
    pub reason: Option<String>,
    pub logout_limitation_accepted: bool,
}
#[derive(serde::Serialize)]
pub struct CollaborationCandidate {
    pub user_id: i32,
    pub username: String,
    pub nickname: String,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AssignmentTargetSummary {
    pub target: AssignmentTarget,
    pub label: String,
    pub secondary: String,
}
#[derive(serde::Serialize)]
pub struct ConfigVersionSummary {
    pub version_id: Uuid,
    pub version: u64,
    pub config: crate::domain::identity::application::EnvironmentConfigInput,
    pub candidate_client_id: Option<String>,
}
#[derive(serde::Serialize)]
pub struct ReaderSummary {
    pub id: Uuid,
    pub reader_id: String,
    pub environment_id: Uuid,
    pub approved_audience: String,
    pub enabled: bool,
    pub revision: u64,
    pub created_at: u64,
}

pub struct InvitationWrite {
    pub id: Uuid,
    pub digest: SecretDigest,
    pub purpose: InvitationPurpose,
    pub target_user_id: Option<i32>,
    pub person_kind: PersonKind,
    pub developer_grant: bool,
    pub created_by: i32,
    pub expires_at: u64,
}

pub struct PasswordAccount {
    pub account: AccountSnapshot,
    pub password_hash: SecretToken,
}

pub struct AccountGovernanceSnapshot {
    pub account: AccountSnapshot,
    pub revision: u64,
    pub enabled_administrator_ids: Vec<i32>,
    pub active_owned_modes: Vec<ManagementMode>,
    pub affected_environments: Vec<Uuid>,
    pub live_rp_session_ids: Vec<Uuid>,
    pub locks: IdentityLockScope,
}

pub struct IdentityRevocationPlan {
    pub locks: IdentityLockScope,
    pub user_ids: std::collections::BTreeSet<i32>,
    pub rp_session_ids: Vec<Uuid>,
}
pub struct ResetWrite {
    pub id: Uuid,
    pub user_id: i32,
    pub digest: SecretDigest,
    pub auth_epoch_at_issue: u64,
    pub created_by: i32,
    pub expires_at: u64,
}
pub struct NewAccountWrite {
    pub username: String,
    pub password_hash: SecretToken,
    pub nickname: String,
    pub person_kind: PersonKind,
    pub developer_enabled: bool,
}
pub struct NewEnvironmentWrite {
    pub id: Uuid,
    pub client_id: String,
    pub client_secret_hash: Option<SecretToken>,
    pub initial_version_id: Uuid,
    pub config: EnvironmentConfig,
}
pub struct RegistrationWrite {
    pub app_key: String,
    pub name: String,
    pub description: Option<String>,
    pub management_mode: ManagementMode,
    pub third_party: bool,
    pub owner_user_id: i32,
    pub development: NewEnvironmentWrite,
    pub production: NewEnvironmentWrite,
}
pub struct LegacyWorkspace {
    pub summary: crate::domain::identity::migration::LegacyClientSummary,
    pub locks: IdentityLockScope,
}
pub struct LegacyAdoptionWrite {
    pub oauth_client_id: i32,
    pub expected_snapshot: String,
    pub existing_application_id: Option<i32>,
    pub retained_environment: crate::domain::identity::application::AppEnvironmentKind,
    pub actor_id: i32,
    pub registration: RegistrationWrite,
}
pub struct VersionWrite {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub expected_revision: u64,
    pub config: EnvironmentConfig,
    pub candidate_client_id: Option<i32>,
    pub actor_id: i32,
}
pub struct ApprovalWrite {
    pub id: Uuid,
    pub snapshot: ApprovalSnapshot,
    pub requested_by: i32,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GroupWorkspace {
    pub group_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub revision: u64,
    pub members: Vec<i32>,
    pub member_profiles: Vec<GroupMemberSummary>,
    pub application_ids: Vec<i32>,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GroupMemberSummary {
    pub user_id: i32,
    pub username: String,
    pub nickname: String,
}
pub struct OperationReceiptWrite {
    pub id: Uuid,
    pub actor_id: i32,
    pub operation_kind: &'static str,
    pub idempotency_key: String,
    pub request_hash: String,
    pub resource_id: String,
    pub summary: serde_json::Value,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct SafeOperationReceipt {
    pub operation_id: Uuid,
    pub request_hash: String,
    pub resource_id: String,
    pub summary: serde_json::Value,
    pub committed_at: u64,
}

/// Commands contain hashes/verified facts rather than raw browser credentials.
pub enum IdentityWrite {
    WithdrawConsent {
        user_id: i32,
        environment_id: Uuid,
    },
    RetryLogout {
        delivery_id: Uuid,
        max_age_seconds: u64,
    },
    RecordLegacyScan {
        oauth_client_id: i32,
    },
    AdoptLegacyApplication(Box<LegacyAdoptionWrite>),
    CreateIntrospectionReader {
        id: Uuid,
        reader_id: String,
        secret_hash: SecretToken,
        environment_id: Uuid,
        approved_audience: String,
        actor_id: i32,
    },
    UpdateIntrospectionReader {
        id: Uuid,
        environment_id: Uuid,
        expected_revision: u64,
        enabled: bool,
        secret_hash: Option<SecretToken>,
    },
    UpdateProfile {
        user_id: i32,
        nickname: String,
        email: Option<String>,
    },
    CreateAccount(NewAccountWrite),
    SetAccount {
        account: AccountSnapshot,
        expected_revision: u64,
    },
    ChangePassword {
        user_id: i32,
        password_hash: SecretToken,
        expected_auth_epoch: u64,
    },
    IssueInvitation(InvitationWrite),
    AcceptInvitation {
        invitation_id: Uuid,
        user_id: i32,
    },
    RevokeInvitation {
        invitation_id: Uuid,
    },
    RevokeReset {
        reset_id: Uuid,
    },
    IssueReset(ResetWrite),
    CreateActionFlow(crate::domain::identity::session::ActionFlowSnapshot),
    ConsumeActionFlow {
        id: Uuid,
        target: crate::domain::identity::session::ActionTarget,
        browser_binding_digest: SecretDigest,
        browser_proof_digest: SecretDigest,
    },
    ConsumeReset {
        reset_id: Uuid,
        expected_auth_epoch: u64,
    },
    SetCandidate {
        requester_user_id: i32,
        candidate_user_id: i32,
        allowed: bool,
        actor_id: i32,
    },
    CreateGroup {
        id: Uuid,
        name: String,
        description: Option<String>,
        actor_id: i32,
    },
    ChangeGroupMember {
        group_id: Uuid,
        user_id: i32,
        present: bool,
        expected_revision: u64,
        actor_id: i32,
    },
    UpdateGroup {
        group_id: Uuid,
        name: String,
        description: Option<String>,
        expected_revision: u64,
    },
    DeleteGroup {
        group_id: Uuid,
        expected_revision: u64,
    },
    RegisterApplication(Box<RegistrationWrite>),
    UpdateApplicationMetadata {
        application_id: i32,
        name: String,
        description: Option<String>,
        expected_revision: u64,
    },
    SetMember {
        application_id: i32,
        user_id: i32,
        present: bool,
        actor_id: i32,
        expected_revision: u64,
    },
    TransferOwner {
        application_id: i32,
        owner_user_id: i32,
        expected_revision: u64,
    },
    InsertVersion(Box<VersionWrite>),
    CreateCandidateClient {
        environment_id: Uuid,
        client_id: String,
        client_secret_hash: Option<SecretToken>,
        config: EnvironmentConfig,
        actor_id: i32,
    },
    RecordAdminActivation {
        snapshot: ApprovalSnapshot,
        actor_id: i32,
    },
    SubmitApproval(Box<ApprovalWrite>),
    ReviewApproval {
        approval_id: Uuid,
        reviewer_id: i32,
        approve: bool,
        reason: Option<String>,
        accept_logout_limitation: bool,
    },
    ResetEnvironmentSecret {
        environment_id: Uuid,
        expected_revision: u64,
        secret_hash: SecretToken,
    },
    ActivateVersion {
        environment_id: Uuid,
        version_id: Uuid,
        expected_revision: u64,
    },
    SetApplicationLifecycle {
        application_id: i32,
        active: bool,
        expected_revision: u64,
    },
    SetAssignment {
        application_id: i32,
        target: AssignmentTarget,
        present: bool,
        actor_id: i32,
    },
    SetTestUser {
        environment_id: Uuid,
        user_id: i32,
        present: bool,
        actor_id: i32,
    },
    RecomputeGeneration {
        environment_id: Uuid,
        user_id: i32,
    },
    EnsureAccessGeneration {
        environment_id: Uuid,
        user_id: i32,
    },
    SaveConsent {
        user_id: i32,
        environment_id: Uuid,
        config_version_id: Uuid,
        scopes: IdentityScopeSet,
    },
    CreateCenter(BrowserSessionSnapshot),
    AttachManagementSession {
        session_id: Uuid,
        sso_session_id: Uuid,
    },
    AttachIdentitySession {
        session_id: Uuid,
        rp_session_id: Uuid,
        admission: ManagedAdmission,
    },
    CreateFlow {
        flow: FlowHandle,
        interactive: bool,
        environment_id: Uuid,
        config_version_id: Uuid,
        browser_binding_digest: SecretDigest,
        request_payload: serde_json::Value,
    },
    BindFlow {
        flow_id: Uuid,
        sso_session_id: Uuid,
        reauthenticated: bool,
    },
    CompleteFlow {
        flow_id: Uuid,
    },
    CreateRelyingParty {
        id: Uuid,
        protocol_sid: Uuid,
        admission: ManagedAdmission,
        nonce: Option<String>,
    },
    CreateCode {
        digest: SecretDigest,
        flow_id: Uuid,
        rp_session_id: Uuid,
        admission: ManagedAdmission,
        verified_request: serde_json::Value,
        expires_at: u64,
    },
    Revoke(IdentityRevocationScope),
    /// Account epoch was advanced by a password/enable write in this transaction.
    RevokeAccountSessions {
        user_id: i32,
    },
    SaveOperation(OperationReceiptWrite),
}

pub enum IdentityWriteResult {
    Account(AccountSnapshot),
    Application {
        application_id: i32,
        development_client_id: String,
        production_client_id: String,
    },
    Resource(Uuid),
    Revision(u64),
    Client {
        oauth_client_id: i32,
        client_id: String,
    },
    Changed,
}

#[derive(Debug, Clone, Copy)]
pub enum AuditResult {
    Success,
    Denied,
    Failure,
}
pub struct AuditEvent {
    pub id: Uuid,
    pub request_id: Uuid,
    pub actor_id: Option<i32>,
    pub action: &'static str,
    pub object_kind: &'static str,
    pub object_id: String,
    pub result: AuditResult,
    pub reason: Option<IdentityError>,
    pub change_summary: serde_json::Value,
    pub operation_id: Option<Uuid>,
    pub commit_marker: Option<Uuid>,
}
pub struct LogoutEvent {
    pub audit_event_id: Uuid,
    pub rp_session_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolRevocationTarget {
    pub issuer: String,
    pub jti: String,
    pub grant_id: Uuid,
    pub client_id: String,
    pub environment_id: Uuid,
    pub user_id: i32,
    pub revoked: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lock_scope_sorts_deduplicates_and_preserves_global_order() {
        let env = Uuid::new_v4();
        let group = Uuid::new_v4();
        let scope = IdentityLockScope::new(vec![
            IdentityLockKey::User(7),
            IdentityLockKey::Group(group),
            IdentityLockKey::User(2),
            IdentityLockKey::Application(1),
            IdentityLockKey::Environment(env),
            IdentityLockKey::User(7),
        ])
        .unwrap();
        assert_eq!(
            scope.keys(),
            &[
                IdentityLockKey::Application(1),
                IdentityLockKey::Environment(env),
                IdentityLockKey::Group(group),
                IdentityLockKey::User(2),
                IdentityLockKey::User(7)
            ]
        );
        assert!(IdentityLockScope::new(vec![IdentityLockKey::User(0)]).is_err());
        assert!(
            IdentityLockScope::new(vec![IdentityLockKey::Credential {
                issuer: String::new(),
                jti: "jti".into()
            }])
            .is_err()
        );
    }
}
