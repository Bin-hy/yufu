use auth::security::error::SecurityError;
use std::fmt;

/// Fixed public classifications; credentials and backend messages are never embedded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    InvalidInput,
    InvalidScope,
    Unauthenticated,
    Forbidden,
    CookieRequiresHttps,
    NotFound,
    Conflict,
    Expired,
    AlreadyConsumed,
    OwnerRequired,
    LastAdministrator,
    ApprovalObsolete,
    SelfApproval,
    LoginRequired,
    ConsentRequired,
    AccessDenied,
    DependencyUnavailable,
    DeadlineExceeded,
    CapacityExceeded,
    CommitUncertain,
}

impl IdentityError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidScope => "invalid_scope",
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden => "forbidden",
            Self::CookieRequiresHttps => "csrf_cookie_requires_https",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Expired => "expired",
            Self::AlreadyConsumed => "already_consumed",
            Self::OwnerRequired => "owner_required",
            Self::LastAdministrator => "last_administrator",
            Self::ApprovalObsolete => "approval_obsolete",
            Self::SelfApproval => "self_approval",
            Self::LoginRequired => "login_required",
            Self::ConsentRequired => "consent_required",
            Self::AccessDenied => "access_denied",
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::CommitUncertain => "commit_uncertain",
        }
    }
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for IdentityError {}

impl From<SecurityError> for IdentityError {
    fn from(value: SecurityError) -> Self {
        match value {
            SecurityError::InvalidCredential | SecurityError::CredentialRevoked => {
                Self::Unauthenticated
            }
            SecurityError::Forbidden => Self::Forbidden,
            SecurityError::InvalidResource => Self::InvalidInput,
            SecurityError::Conflict => Self::Conflict,
            SecurityError::DependencyUnavailable => Self::DependencyUnavailable,
            SecurityError::DeadlineExceeded => Self::DeadlineExceeded,
            SecurityError::CapacityExceeded => Self::CapacityExceeded,
            SecurityError::CommitUncertain => Self::CommitUncertain,
        }
    }
}

impl From<IdentityError> for SecurityError {
    fn from(value: IdentityError) -> Self {
        match value {
            IdentityError::Unauthenticated
            | IdentityError::Expired
            | IdentityError::AlreadyConsumed
            | IdentityError::LoginRequired => Self::InvalidCredential,
            IdentityError::Conflict | IdentityError::ApprovalObsolete => Self::Conflict,
            IdentityError::DeadlineExceeded => Self::DeadlineExceeded,
            IdentityError::CapacityExceeded => Self::CapacityExceeded,
            IdentityError::CommitUncertain => Self::CommitUncertain,
            IdentityError::DependencyUnavailable => Self::DependencyUnavailable,
            IdentityError::NotFound | IdentityError::InvalidInput | IdentityError::InvalidScope => {
                Self::InvalidResource
            }
            _ => Self::Forbidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_has_fixed_safe_code() {
        assert_eq!(IdentityError::SelfApproval.to_string(), "self_approval");
        assert_eq!(
            IdentityError::from(SecurityError::DeadlineExceeded),
            IdentityError::DeadlineExceeded
        );
        assert_eq!(
            IdentityError::from(SecurityError::CredentialRevoked),
            IdentityError::Unauthenticated
        );
    }
}
