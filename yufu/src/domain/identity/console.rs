//! Closed console queries and public DTOs; never serialize persistence entities.
use super::error::IdentityError;
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum ConsoleKind {
    Applications,
    Consents,
    Centers,
    RelyingParties,
    Accounts,
    Audit,
    Deliveries,
    AppDeliveries,
}
pub enum ConsoleQuery {
    Applications {
        user_id: i32,
        root_id: Uuid,
        after: Option<Uuid>,
        limit: u32,
        environment: Option<super::application::AppEnvironmentKind>,
        search: Option<String>,
    },
    Consents {
        user_id: i32,
        after: i32,
        limit: u32,
    },
    Centers {
        user_id: i32,
        current_id: Uuid,
        after: Option<Uuid>,
        limit: u32,
    },
    RelyingParties {
        user_id: i32,
        after: Option<Uuid>,
        limit: u32,
    },
    Accounts {
        after: i32,
        limit: u32,
        search: Option<String>,
        enabled: Option<bool>,
    },
    Audit {
        after: Option<Uuid>,
        limit: u32,
    },
    Deliveries {
        after: Option<Uuid>,
        limit: u32,
        status: Option<String>,
    },
    /// Aggregated administrator counters; every section is gated by its own
    /// read operation and omitted (None) when the actor lacks that grant.
    Overview {
        accounts: bool,
        deliveries: bool,
        approvals: bool,
        migrations: bool,
        /// Lower bound (epoch milliseconds) for the audit failure counter.
        audit_since: Option<i64>,
    },
    AppDeliveries {
        application_id: i32,
        environment_id: Uuid,
        after: Option<Uuid>,
        limit: u32,
    },
}
#[derive(Serialize)]
pub struct ConsolePage<T, C> {
    pub items: Vec<T>,
    pub next: Option<C>,
}
pub fn page<T, C: Copy>(
    mut items: Vec<T>,
    limit: u32,
    cursor: impl Fn(&T) -> C,
) -> ConsolePage<T, C> {
    let has_more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next = if has_more {
        items.last().map(cursor)
    } else {
        None
    };
    ConsolePage { items, next }
}
pub fn validate_limit(limit: u32) -> Result<(), IdentityError> {
    if (1..=100).contains(&limit) {
        Ok(())
    } else {
        Err(IdentityError::InvalidInput)
    }
}
/// The closed set of delivery states the outbox writes; filters outside it are rejected.
pub const DELIVERY_STATUSES: &[&str] = &[
    "pending",
    "leased",
    "retry",
    "delivered",
    "permanent_failure",
    "expired",
];
pub fn validate_delivery_status(status: Option<&str>) -> Result<Option<String>, IdentityError> {
    status
        .map(|value| {
            if DELIVERY_STATUSES.contains(&value) {
                Ok(value.to_owned())
            } else {
                Err(IdentityError::InvalidInput)
            }
        })
        .transpose()
}
#[derive(Serialize)]
#[serde(untagged)]
pub enum ConsoleResult {
    Applications(ConsolePage<PersonalApplication, Uuid>),
    Consents(ConsolePage<PersonalConsent, i32>),
    Centers(ConsolePage<PersonalCenter, Uuid>),
    RelyingParties(ConsolePage<PersonalRelyingParty, Uuid>),
    Accounts(ConsolePage<ConsoleAccount, i32>),
    Audit(ConsolePage<ConsoleAudit, Uuid>),
    Deliveries(ConsolePage<ConsoleDelivery, Uuid>),
    Overview(ConsoleOverview),
}
#[derive(Serialize)]
pub struct ConsoleOverview {
    pub accounts: Option<AccountOverview>,
    pub deliveries: Option<DeliveryOverview>,
    pub approvals: Option<ApprovalOverview>,
    pub migrations: Option<MigrationOverview>,
    pub audit: Option<AuditOverview>,
}
#[derive(Serialize)]
pub struct AccountOverview {
    pub total: u64,
    pub disabled: u64,
    pub developers: u64,
}
#[derive(Serialize)]
pub struct DeliveryOverview {
    pub pending: u64,
    pub leased: u64,
    pub retry: u64,
    pub delivered: u64,
    pub permanent_failure: u64,
    pub expired: u64,
}
#[derive(Serialize)]
pub struct ApprovalOverview {
    pub pending: u64,
}
#[derive(Serialize)]
pub struct MigrationOverview {
    pub unmanaged: u64,
    pub unresolved_conflicts: u64,
}
#[derive(Serialize)]
pub struct AuditOverview {
    /// Events inside the requested window whose result is not success.
    pub recent_failures: u64,
}
#[derive(Serialize)]
pub struct PersonalApplication {
    pub environment_id: Uuid,
    pub application_id: i32,
    pub name: String,
    pub environment: String,
    pub client_id: String,
    pub launchable: bool,
    pub launch_description: Option<String>,
    pub sources: Vec<super::access::AccessSource>,
}
#[derive(Serialize)]
pub struct PersonalConsent {
    pub id: i32,
    pub application_id: i32,
    pub name: String,
    pub environment_id: Uuid,
    pub environment: String,
    pub version_id: Uuid,
    pub scopes: Vec<String>,
    pub current_version: bool,
}
#[derive(Serialize)]
pub struct PersonalCenter {
    pub id: Uuid,
    pub current: bool,
    pub auth_time: u64,
    pub expires_at: u64,
    pub revoked_at: Option<u64>,
}
#[derive(Serialize)]
pub struct PersonalRelyingParty {
    pub id: Uuid,
    pub center_id: Uuid,
    pub name: String,
    pub environment: String,
    pub created_at: u64,
    pub revoked_at: Option<u64>,
    pub center_expires_at: u64,
    pub center_revoked: bool,
}
#[derive(Serialize)]
pub struct ConsoleAccount {
    pub user_id: i32,
    pub username: String,
    pub nickname: String,
    pub enabled: bool,
    pub person_kind: String,
    pub developer_enabled: bool,
    pub revision: u64,
}
#[derive(Serialize)]
pub struct ConsoleAudit {
    pub id: Uuid,
    pub request_id: Uuid,
    pub actor_id: Option<i32>,
    pub action: String,
    pub object_kind: String,
    pub object_id: String,
    pub happened_at: u64,
    pub result: String,
    pub reason_code: Option<String>,
    pub change_summary: serde_json::Value,
}
#[derive(Serialize)]
pub struct ConsoleDelivery {
    pub id: Uuid,
    pub event_id: Uuid,
    pub application_id: i32,
    pub name: String,
    pub environment: String,
    pub status: String,
    pub attempt_count: u32,
    pub next_attempt_at: u64,
    pub last_error_code: Option<String>,
    pub created_at: u64,
    pub delivered_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn console_pagination_keeps_last_visible_cursor_and_bounds() {
        let result = page(vec![1, 2, 3], 2, |v| *v);
        assert_eq!(result.items, vec![1, 2]);
        assert_eq!(result.next, Some(2));
        assert_eq!(page(vec![1, 2], 2, |v| *v).next, None);
        assert_eq!(page(Vec::<i32>::new(), 2, |v| *v).next, None);
        assert!(validate_limit(0).is_err());
        assert!(validate_limit(101).is_err());
        assert!(validate_limit(100).is_ok());
    }
}
