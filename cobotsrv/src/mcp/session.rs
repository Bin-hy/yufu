//! Active MCP session tracking.
//!
//! MCP Streamable HTTP identifies a session via the `Mcp-Session-Id` header.
//! We register a session on `initialize` (generating an id if the client did
//! not supply one), refresh `last_seen` on every subsequent request that
//! carries the id, and a background reaper evicts sessions idle past a TTL.
//! "Active sessions" therefore means: initialized and seen within the TTL.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Serialize;

/// Maximum number of concurrent sessions the registry will hold. Beyond this,
/// new `initialize` requests are refused (HTTP 503) rather than evicting
/// arbitrary live sessions.
pub const MAX_SESSIONS: usize = 10_000;

/// Transport a session is using.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Http,
    Sse,
}

#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub subject: String,
    pub connected_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub transport: Transport,
}

/// View object returned by the admin API.
#[derive(Clone, Debug, Serialize)]
pub struct SessionVO {
    pub session_id: String,
    pub subject: String,
    pub connected_at: String,
    pub last_seen: String,
    pub transport: Transport,
}

/// Outcome of refreshing an existing session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchResult {
    /// Known session, subject matches — `last_seen` refreshed.
    Ok,
    /// No session with this id is registered.
    Unknown,
    /// The session exists but belongs to a different subject.
    SubjectMismatch,
}

/// Concurrent registry of active MCP sessions, keyed by `Mcp-Session-Id`.
#[derive(Default)]
pub struct SessionRegistry {
    sessions: DashMap<String, SessionInfo>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    /// Register a newly initialized session. Returns `false` (and logs an
    /// error) when the registry is at capacity — the caller should reject
    /// the request with 503.
    pub fn register(&self, session_id: &str, subject: &str, transport: Transport) -> bool {
        if !self.sessions.contains_key(session_id) && self.sessions.len() >= MAX_SESSIONS {
            tracing::error!(
                max_sessions = MAX_SESSIONS,
                "mcp session registry at capacity; refusing new session"
            );
            return false;
        }
        let now = Utc::now();
        self.sessions.insert(
            session_id.to_string(),
            SessionInfo {
                subject: subject.to_string(),
                connected_at: now,
                last_seen: now,
                transport,
            },
        );
        true
    }

    /// Refresh `last_seen` for a known session. Never inserts — unknown ids
    /// and subject mismatches are reported so the caller can reject them
    /// (MCP spec: unknown session ids get HTTP 404).
    pub fn touch(&self, session_id: &str, subject: &str, transport: Transport) -> TouchResult {
        let now = Utc::now();
        match self.sessions.get_mut(session_id) {
            Some(mut entry) => {
                if !entry.subject.is_empty() && !subject.is_empty() && entry.subject != subject {
                    tracing::warn!(
                        session_id,
                        existing_subject = %entry.subject,
                        incoming_subject = %subject,
                        "mcp session touch rejected: subject mismatch"
                    );
                    return TouchResult::SubjectMismatch;
                }
                entry.last_seen = now;
                entry.transport = transport;
                if entry.subject.is_empty() {
                    entry.subject = subject.to_string();
                }
                TouchResult::Ok
            }
            None => TouchResult::Unknown,
        }
    }

    /// Snapshot of all sessions, newest activity first.
    pub fn list(&self) -> Vec<SessionVO> {
        let mut out: Vec<SessionVO> = self
            .sessions
            .iter()
            .map(|e| SessionVO {
                session_id: e.key().clone(),
                subject: e.subject.clone(),
                connected_at: e
                    .connected_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                last_seen: e
                    .last_seen
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                transport: e.transport,
            })
            .collect();
        out.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        out
    }

    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }

    /// Evict sessions idle longer than `ttl`.
    fn reap(&self, ttl: Duration) {
        let cutoff = Utc::now() - chrono::Duration::from_std(ttl).unwrap_or_default();
        self.sessions.retain(|_, s| s.last_seen >= cutoff);
    }

    /// Spawn a background task that evicts idle sessions every `interval`.
    pub fn spawn_reaper(self: &Arc<Self>, ttl: Duration, interval: Duration) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tick.tick().await;
                this.reap(ttl);
            }
        });
    }
}
