use std::fmt;

/// Safe-to-display classifications. Never embed raw credentials or backend errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityError {
    InvalidCredential,
    CredentialRevoked,
    Forbidden,
    InvalidResource,
    Conflict,
    DependencyUnavailable,
    DeadlineExceeded,
    CapacityExceeded,
    CommitUncertain,
}

impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidCredential => "invalid credential",
            Self::CredentialRevoked => "credential revoked",
            Self::Forbidden => "operation forbidden",
            Self::InvalidResource => "invalid resource",
            Self::Conflict => "request conflict",
            Self::DependencyUnavailable => "security dependency unavailable",
            Self::DeadlineExceeded => "security deadline exceeded",
            Self::CapacityExceeded => "security capacity exceeded",
            Self::CommitUncertain => "security commit uncertain",
        })
    }
}

impl std::error::Error for SecurityError {}
