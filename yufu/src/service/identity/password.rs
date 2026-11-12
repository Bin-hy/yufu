//! CPU-bound password hashing is intentionally decoupled from the database
//! security budget. Bcrypt cost 12 can exceed the 2-second transaction budget
//! when the host is under load, which used to turn a valid login into 503.
use crate::domain::identity::error::IdentityError;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Semaphore;

/// A bounded CPU allowance for one hash or verification. This is not a
/// database deadline: the subsequent write still gets its own 2-second budget.
const HASH_TIMEOUT: Duration = Duration::from_secs(10);

fn limiter() -> &'static Semaphore {
    static LIMITER: OnceLock<Semaphore> = OnceLock::new();
    LIMITER.get_or_init(|| {
        let permits = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(2)
            .clamp(1, 8);
        Semaphore::new(permits)
    })
}

async fn run<T, F>(work: F) -> Result<T, IdentityError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, bcrypt::BcryptError> + Send + 'static,
{
    let deadline = tokio::time::Instant::now() + HASH_TIMEOUT;
    let permit = tokio::time::timeout_at(deadline, limiter().acquire())
        .await
        .map_err(|_| IdentityError::DeadlineExceeded)?
        .map_err(|_| IdentityError::DependencyUnavailable)?;
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    tokio::task::spawn_blocking(move || {
        let result = work();
        let _ = sender.send(result);
        // Keep the permit until the blocking work really finishes. Dropping a
        // timed-out JoinHandle does not cancel the thread, so releasing the
        // permit early would let a saturated host queue unbounded work.
        drop(permit);
    });
    match tokio::time::timeout(remaining, receiver).await {
        Ok(Ok(result)) => result.map_err(|_| IdentityError::DependencyUnavailable),
        Ok(Err(_)) => Err(IdentityError::DependencyUnavailable),
        Err(_) => Err(IdentityError::DeadlineExceeded),
    }
}

pub(crate) async fn verify(password: String, hash: String) -> Result<bool, IdentityError> {
    run(move || bcrypt::verify(password, &hash)).await
}

pub(crate) async fn hash(password: String) -> Result<String, IdentityError> {
    run(move || bcrypt::hash(password, bcrypt::DEFAULT_COST)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_and_verifies_without_a_database_budget() {
        let hash = hash("fixture-password".into()).await.unwrap();
        assert!(
            verify("fixture-password".into(), hash.clone())
                .await
                .unwrap()
        );
        assert!(!verify("wrong-password".into(), hash).await.unwrap());
    }
}
