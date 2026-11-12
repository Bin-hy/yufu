//! Explicit, method-aware enterprise management operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityOperation {
    InvitationIssue,
    InvitationRevoke,
    PasswordResetIssue,
    PasswordResetRevoke,
    AccountRead,
    AccountEnable,
    AccountDeveloper,
    ApplicationRead,
    ApplicationConfigure,
    ApplicationReview,
    ApplicationLifecycle,
    ApplicationOwner,
    CollaborationCandidate,
    GroupRead,
    GroupCreate,
    GroupConfigure,
    GroupMember,
    GroupDelete,
    ApplicationAssignment,
    ApplicationTestUser,
    ApplicationRegister,
    ApplicationActivate,
    ApplicationMetadata,
    ApplicationSecret,
    ReaderRead,
    ReaderCreate,
    ReaderChange,
    MigrationRead,
    MigrationScan,
    MigrationAdopt,
    AuditRead,
    DeliveryRead,
    DeliveryRetry,
}
impl IdentityOperation {
    pub const ALL: &[Self] = &[
        Self::InvitationIssue,
        Self::InvitationRevoke,
        Self::PasswordResetIssue,
        Self::PasswordResetRevoke,
        Self::AccountRead,
        Self::AccountEnable,
        Self::AccountDeveloper,
        Self::ApplicationRead,
        Self::ApplicationConfigure,
        Self::ApplicationReview,
        Self::ApplicationLifecycle,
        Self::ApplicationOwner,
        Self::CollaborationCandidate,
        Self::GroupRead,
        Self::GroupCreate,
        Self::GroupConfigure,
        Self::GroupMember,
        Self::GroupDelete,
        Self::ApplicationAssignment,
        Self::ApplicationTestUser,
        Self::ApplicationRegister,
        Self::ApplicationActivate,
        Self::ApplicationMetadata,
        Self::ApplicationSecret,
        Self::ReaderRead,
        Self::ReaderCreate,
        Self::ReaderChange,
        Self::MigrationRead,
        Self::MigrationScan,
        Self::MigrationAdopt,
        Self::AuditRead,
        Self::DeliveryRead,
        Self::DeliveryRetry,
    ];
    pub fn registration(self) -> (&'static str, &'static str) {
        match self {
            Self::AuditRead => ("GET", "/admin/audit-events"),
            Self::DeliveryRead => ("GET", "/admin/logout-deliveries"),
            Self::DeliveryRetry => ("POST", "/admin/logout-deliveries/{id}/retry"),
            Self::MigrationRead => ("GET", "/admin/identity/migrations"),
            Self::MigrationScan => ("POST", "/admin/identity/migrations/scan"),
            Self::MigrationAdopt => ("POST", "/admin/identity/migrations/{id}/adopt"),
            Self::InvitationIssue => ("POST", "/admin/invitations"),
            Self::InvitationRevoke => ("POST", "/admin/invitations/{id}/revoke"),
            Self::PasswordResetIssue => ("POST", "/admin/password-resets"),
            Self::PasswordResetRevoke => ("POST", "/admin/password-resets/{id}/revoke"),
            Self::AccountRead => ("GET", "/admin/accounts"),
            Self::AccountEnable => ("PATCH", "/admin/accounts/{id}/enabled"),
            Self::AccountDeveloper => ("PATCH", "/admin/accounts/{id}/developer"),
            Self::ApplicationRead => ("GET", "/admin/apps"),
            Self::ReaderRead => (
                "GET",
                "/admin/apps/{id}/environments/{env}/integration/readers",
            ),
            Self::ReaderCreate => (
                "POST",
                "/admin/apps/{id}/environments/{env}/integration/readers",
            ),
            Self::ReaderChange => (
                "PATCH",
                "/admin/apps/{id}/environments/{env}/integration/readers/{reader_id}",
            ),
            Self::ApplicationConfigure => ("PUT", "/admin/apps/{id}/environments/{env}/config"),
            Self::ApplicationRegister => ("POST", "/admin/apps"),
            Self::ApplicationActivate => ("POST", "/admin/apps/{id}/environments/{env}/activate"),
            Self::ApplicationMetadata => ("PATCH", "/admin/apps/{id}"),
            Self::ApplicationSecret => ("POST", "/admin/apps/{id}/environments/{env}/secret/reset"),
            Self::ApplicationReview => ("POST", "/admin/apps/{id}/approvals/{approval_id}/review"),
            Self::ApplicationLifecycle => ("PATCH", "/admin/apps/{id}/lifecycle"),
            Self::ApplicationOwner => ("PATCH", "/admin/apps/{id}/owner"),
            Self::CollaborationCandidate => ("PATCH", "/admin/collaboration-candidates"),
            Self::GroupRead => ("GET", "/admin/groups"),
            Self::GroupCreate => ("POST", "/admin/groups"),
            Self::GroupConfigure => ("PATCH", "/admin/groups/{id}"),
            Self::GroupMember => ("PATCH", "/admin/groups/{id}/members/{user_id}"),
            Self::GroupDelete => ("DELETE", "/admin/groups/{id}"),
            Self::ApplicationAssignment => ("PATCH", "/admin/apps/{id}/assignments"),
            Self::ApplicationTestUser => (
                "PATCH",
                "/admin/apps/{id}/environments/{env}/test-users/{user_id}",
            ),
        }
    }
    /// Only used when the operation is first registered during upgrade.
    /// Existing edited registrations and grants are left alone.
    pub fn legacy_registration(self) -> (&'static str, &'static str) {
        match self {
            Self::AuditRead | Self::DeliveryRead => ("GET", "/recordings"),
            Self::DeliveryRetry => ("PATCH", "/application"),
            Self::MigrationRead => ("GET", "/oidc/clients"),
            Self::MigrationScan | Self::MigrationAdopt => ("PATCH", "/application"),
            Self::InvitationIssue | Self::InvitationRevoke => ("POST", "/user"),
            Self::PasswordResetIssue | Self::PasswordResetRevoke | Self::AccountEnable => {
                ("PATCH", "/user")
            }
            Self::AccountRead => ("GET", "/users"),
            Self::AccountDeveloper => ("PATCH", "/user/role"),
            Self::ApplicationRead => ("GET", "/applications"),
            Self::ReaderRead => ("GET", "/application"),
            Self::ReaderCreate | Self::ReaderChange => ("PATCH", "/application"),
            Self::ApplicationConfigure => ("POST", "/oidc/clients"),
            Self::ApplicationRegister => ("POST", "/oidc/clients"),
            Self::ApplicationActivate | Self::ApplicationMetadata | Self::ApplicationSecret => {
                ("PATCH", "/application")
            }
            Self::ApplicationReview | Self::ApplicationLifecycle | Self::ApplicationOwner => {
                ("PATCH", "/application")
            }
            Self::CollaborationCandidate => ("PATCH", "/user/role"),
            Self::GroupRead => ("GET", "/users"),
            Self::GroupCreate | Self::GroupConfigure | Self::GroupMember | Self::GroupDelete => {
                ("PATCH", "/user/role")
            }
            Self::ApplicationAssignment | Self::ApplicationTestUser => ("PATCH", "/application"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_operations_are_explicit_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for operation in IdentityOperation::ALL {
            let (method, route) = operation.registration();
            assert!(seen.insert((method, route)));
            assert!(route.starts_with("/admin/"));
            assert!(["GET", "POST", "PATCH", "PUT", "DELETE"].contains(&method));
        }
        assert_ne!(
            IdentityOperation::InvitationIssue.registration(),
            IdentityOperation::InvitationRevoke.registration()
        );
    }
}
