use super::repository::{IdentityLockKey, IdentityLockScope, IdentityTransaction};
use crate::domain::identity::{
    account::{SecretDigest, VerifiedActor},
    error::IdentityError,
};

pub use super::repository_account::initialize_provider;
#[path = "account_actions.rs"]
mod actions;
pub use actions::*;
#[path = "account_governance.rs"]
mod governance;
#[path = "account_password.rs"]
mod password;
pub use governance::AccountChangeResult;

/// Reload after acquiring account/root barriers; a pre-lock guard is never
/// sufficient authorization for a later write.
pub async fn authenticate_current_actor(
    transaction: &mut dyn IdentityTransaction,
    digest: &SecretDigest,
    now_millis: u64,
    business_scope: &IdentityLockScope,
) -> Result<VerifiedActor, IdentityError> {
    let initial = transaction
        .load_browser_session(digest)
        .await?
        .ok_or(IdentityError::Unauthenticated)?;
    let mut keys = business_scope.keys().to_vec();
    keys.extend([
        IdentityLockKey::User(initial.user_id),
        IdentityLockKey::Center(initial.id),
    ]);
    transaction
        .lock_identity_scope(&IdentityLockScope::new(keys)?)
        .await?;
    let root = transaction
        .load_browser_session(digest)
        .await?
        .ok_or(IdentityError::Unauthenticated)?;
    let account = transaction
        .load_account(root.user_id)
        .await?
        .ok_or(IdentityError::Unauthenticated)?;
    root.validate(&account, now_millis)?;
    VerifiedActor::verified(account, root.principal.issuer(), root.id)
}

/// A read-only actor from the current primary snapshot. Writes must use the
/// lock-and-reload helper above instead of carrying this preflight into a write.
pub async fn read_current_actor(
    transaction: &mut dyn IdentityTransaction,
    digest: &SecretDigest,
    now_millis: u64,
) -> Result<VerifiedActor, IdentityError> {
    let root = transaction
        .load_browser_session(digest)
        .await?
        .ok_or(IdentityError::Unauthenticated)?;
    let account = transaction
        .load_account(root.user_id)
        .await?
        .ok_or(IdentityError::Unauthenticated)?;
    root.validate(&account, now_millis)?;
    VerifiedActor::verified(account, root.principal.issuer(), root.id)
}
