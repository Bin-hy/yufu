use super::error::IdentityError;
use auth::security::identity::{PrincipalKind, PrincipalRef};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonKind {
    Employee,
    Collaborator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagementMode {
    DeveloperManaged,
    AdminConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvitationPurpose {
    Onboarding,
    Qualification,
}

#[derive(Debug, Clone)]
pub struct InvitationSnapshot {
    pub id: Uuid,
    pub purpose: InvitationPurpose,
    pub target_user_id: Option<i32>,
    pub person_kind: PersonKind,
    pub developer_grant: bool,
    pub expires_at: u64,
    pub consumed_at: Option<u64>,
    pub revoked_at: Option<u64>,
}
impl InvitationSnapshot {
    pub fn validate(
        &self,
        now_millis: u64,
        accepting_user: Option<i32>,
    ) -> Result<(), IdentityError> {
        if self.revoked_at.is_some() {
            return Err(IdentityError::AccessDenied);
        }
        if self.consumed_at.is_some() {
            return Err(IdentityError::AlreadyConsumed);
        }
        if now_millis >= self.expires_at {
            return Err(IdentityError::Expired);
        }
        if self.target_user_id.is_some() && self.target_user_id != accepting_user {
            return Err(IdentityError::Forbidden);
        }
        if self.purpose == InvitationPurpose::Qualification && accepting_user.is_none() {
            return Err(IdentityError::Unauthenticated);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PasswordResetSnapshot {
    pub id: Uuid,
    pub user_id: i32,
    pub auth_epoch_at_issue: u64,
    pub expires_at: u64,
    pub consumed_at: Option<u64>,
    pub revoked_at: Option<u64>,
}
impl PasswordResetSnapshot {
    pub fn validate(
        &self,
        account: &AccountSnapshot,
        now_millis: u64,
    ) -> Result<(), IdentityError> {
        ensure_enabled(account)?;
        if self.revoked_at.is_some() {
            return Err(IdentityError::AccessDenied);
        }
        if self.consumed_at.is_some() {
            return Err(IdentityError::AlreadyConsumed);
        }
        if now_millis >= self.expires_at {
            return Err(IdentityError::Expired);
        }
        if self.user_id != account.user_id || self.auth_epoch_at_issue != account.auth_epoch {
            return Err(IdentityError::Forbidden);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSnapshot {
    pub user_id: i32,
    pub enabled: bool,
    pub person_kind: PersonKind,
    pub developer_enabled: bool,
    pub administrator: bool,
    pub auth_epoch: u64,
}

/// Constructible only by the server after current authentication and account checks.
/// This is deliberately not a request DTO.
#[derive(Clone)]
pub struct VerifiedActor {
    account: AccountSnapshot,
    principal: PrincipalRef,
    sso_session_id: Uuid,
}

impl VerifiedActor {
    pub(crate) fn verified(
        account: AccountSnapshot,
        issuer: &str,
        sso_session_id: Uuid,
    ) -> Result<Self, IdentityError> {
        ensure_enabled(&account)?;
        Ok(Self {
            principal: stable_principal(issuer, account.user_id)?,
            account,
            sso_session_id,
        })
    }
    pub fn account(&self) -> &AccountSnapshot {
        &self.account
    }
    pub fn principal(&self) -> &PrincipalRef {
        &self.principal
    }
    pub fn sso_session_id(&self) -> Uuid {
        self.sso_session_id
    }
}

/// A secret has neither Debug, Display nor a default serialization implementation.
/// The response adapter must consume it, so a receipt cannot disclose it twice.
///
/// ```compile_fail
/// use cobotusr::domain::identity::account::IssuedSecret;
/// fn log(secret: IssuedSecret) { println!("{secret:?}"); }
/// ```
pub struct IssuedSecret(String);

/// Passwords preserve spaces and enforce bcrypt's byte limit before hashing.
pub struct AccountPassword(String);
impl AccountPassword {
    pub fn new(value: String) -> Result<Self, IdentityError> {
        if value.is_empty() || value.len() > 72 {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self(value))
    }
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for AccountPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccountPassword(REDACTED)")
    }
}

impl IssuedSecret {
    /// Entropy is supplied by the infrastructure's operating-system RNG.
    pub fn from_entropy(entropy: [u8; 32]) -> Self {
        Self(URL_SAFE_NO_PAD.encode(entropy))
    }
    pub fn digest(&self) -> SecretDigest {
        SecretDigest::of(&self.0)
    }
    pub fn into_response(self) -> String {
        self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SecretDigest([u8; 32]);

impl SecretDigest {
    pub fn of(secret: &str) -> Self {
        Self(Sha256::digest(secret.as_bytes()).into())
    }
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn hex(&self) -> String {
        hex::encode(self.0)
    }
    pub fn matches(&self, secret: &str) -> bool {
        self == &Self::of(secret)
    }
}

impl std::fmt::Debug for SecretDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretDigest(REDACTED)")
    }
}

pub fn stable_principal(issuer: &str, user_id: i32) -> Result<PrincipalRef, IdentityError> {
    if user_id <= 0 {
        return Err(IdentityError::InvalidInput);
    }
    PrincipalRef::new(issuer, PrincipalKind::User, user_id.to_string()).map_err(Into::into)
}

pub fn ensure_enabled(account: &AccountSnapshot) -> Result<(), IdentityError> {
    if account.user_id <= 0 {
        return Err(IdentityError::InvalidInput);
    }
    if !account.enabled {
        return Err(IdentityError::Unauthenticated);
    }
    Ok(())
}

pub fn owner_is_qualified(account: &AccountSnapshot, mode: ManagementMode) -> bool {
    account.user_id > 0
        && account.enabled
        && match mode {
            ManagementMode::DeveloperManaged => account.developer_enabled,
            ManagementMode::AdminConfigured => account.administrator,
        }
}

/// `active_owned_modes` and administrator count must come from a locked current snapshot.
pub fn validate_account_change(
    current: &AccountSnapshot,
    proposed: &AccountSnapshot,
    enabled_administrators: usize,
    active_owned_modes: &[ManagementMode],
) -> Result<(), IdentityError> {
    if current.user_id != proposed.user_id
        || current.user_id <= 0
        || current.person_kind != proposed.person_kind
        || current.auth_epoch != proposed.auth_epoch
    {
        return Err(IdentityError::InvalidInput);
    }
    if current.enabled
        && current.administrator
        && (!proposed.enabled || !proposed.administrator)
        && enabled_administrators <= 1
    {
        return Err(IdentityError::LastAdministrator);
    }
    if active_owned_modes
        .iter()
        .any(|mode| !owner_is_qualified(proposed, *mode))
    {
        return Err(IdentityError::OwnerRequired);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_account_password_preserves_spaces_and_enforces_bcrypt_byte_limit() {
        let password = AccountPassword::new("  a password with spaces  ".into()).unwrap();
        assert_eq!(password.expose_secret(), "  a password with spaces  ");
        assert!(!format!("{password:?}").contains(password.expose_secret()));
        assert!(AccountPassword::new("a".repeat(72)).is_ok());
        assert!(AccountPassword::new("a".repeat(73)).is_err());
        assert!(AccountPassword::new("密".repeat(25)).is_err());
        assert!(AccountPassword::new(String::new()).is_err());
    }
    fn account() -> AccountSnapshot {
        AccountSnapshot {
            user_id: 7,
            enabled: true,
            person_kind: PersonKind::Collaborator,
            developer_enabled: false,
            administrator: true,
            auth_epoch: 2,
        }
    }

    #[test]
    fn test_invitation_and_reset_require_current_target_epoch_and_unused_source() {
        let mut invitation = InvitationSnapshot {
            id: Uuid::new_v4(),
            purpose: InvitationPurpose::Qualification,
            target_user_id: Some(7),
            person_kind: PersonKind::Collaborator,
            developer_grant: true,
            expires_at: 1_000,
            consumed_at: None,
            revoked_at: None,
        };
        assert_eq!(invitation.validate(999, Some(7)), Ok(()));
        assert_eq!(
            invitation.validate(999, Some(8)),
            Err(IdentityError::Forbidden)
        );
        assert_eq!(
            invitation.validate(1_000, Some(7)),
            Err(IdentityError::Expired)
        );
        invitation.consumed_at = Some(900);
        assert_eq!(
            invitation.validate(999, Some(7)),
            Err(IdentityError::AlreadyConsumed)
        );
        let mut reset = PasswordResetSnapshot {
            id: Uuid::new_v4(),
            user_id: 7,
            auth_epoch_at_issue: 2,
            expires_at: 1_000,
            consumed_at: None,
            revoked_at: None,
        };
        assert_eq!(reset.validate(&account(), 999), Ok(()));
        reset.auth_epoch_at_issue = 1;
        assert_eq!(
            reset.validate(&account(), 999),
            Err(IdentityError::Forbidden)
        );
        reset.auth_epoch_at_issue = 2;
        reset.revoked_at = Some(900);
        assert_eq!(
            reset.validate(&account(), 999),
            Err(IdentityError::AccessDenied)
        );
    }

    #[test]
    fn test_owner_qualification_is_independent_of_person_kind_and_admin() {
        let mut a = account();
        assert!(owner_is_qualified(&a, ManagementMode::AdminConfigured));
        assert!(!owner_is_qualified(&a, ManagementMode::DeveloperManaged));
        a.developer_enabled = true;
        assert!(owner_is_qualified(&a, ManagementMode::DeveloperManaged));
        a.enabled = false;
        assert!(!owner_is_qualified(&a, ManagementMode::DeveloperManaged));
    }

    #[test]
    fn test_account_change_protects_last_admin_and_active_owner() {
        let a = account();
        let mut next = a.clone();
        next.enabled = false;
        assert_eq!(
            validate_account_change(&a, &next, 1, &[]),
            Err(IdentityError::LastAdministrator)
        );
        assert_eq!(
            validate_account_change(&a, &next, 2, &[ManagementMode::AdminConfigured]),
            Err(IdentityError::OwnerRequired)
        );
        assert!(validate_account_change(&a, &next, 2, &[]).is_ok());
        next.user_id = 9;
        assert_eq!(
            validate_account_change(&a, &next, 2, &[]),
            Err(IdentityError::InvalidInput)
        );
    }

    #[test]
    fn test_revoking_developer_requires_owner_transfer() {
        let mut a = account();
        a.developer_enabled = true;
        let mut next = a.clone();
        next.developer_enabled = false;
        assert_eq!(
            validate_account_change(&a, &next, 2, &[ManagementMode::DeveloperManaged]),
            Err(IdentityError::OwnerRequired)
        );
        assert!(validate_account_change(&a, &next, 2, &[ManagementMode::AdminConfigured]).is_ok());
    }

    #[test]
    fn test_stable_subject_and_verified_actor() {
        let a = account();
        let sid = Uuid::new_v4();
        let actor = VerifiedActor::verified(a.clone(), "https://id.example", sid).unwrap();
        assert_eq!(actor.principal().subject(), "7");
        assert_eq!(actor.account(), &a);
        assert_eq!(actor.sso_session_id(), sid);
        assert!(stable_principal("https://id.example", 0).is_err());
        let mut disabled = a;
        disabled.enabled = false;
        assert!(VerifiedActor::verified(disabled, "https://id.example", sid).is_err());
    }

    #[test]
    fn test_issued_secret_hash_and_safe_debug() {
        let secret = IssuedSecret::from_entropy([42; 32]);
        let digest = secret.digest();
        let raw = secret.into_response();
        assert_eq!(raw.len(), 43);
        assert!(digest.matches(&raw));
        assert!(!digest.matches("different"));
        assert_eq!(digest, SecretDigest::from_bytes(*digest.as_bytes()));
        assert_eq!(digest.hex().len(), 64);
        assert_eq!(format!("{digest:?}"), "SecretDigest(REDACTED)");
        assert!(!format!("{digest:?}").contains(&raw));
    }
}
