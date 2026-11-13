use super::{console::ConsoleAudit, error::IdentityError};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditFilterInput {
    pub from: Option<String>,
    pub to: Option<String>,
    pub actor: Option<String>,
    pub action: Option<String>,
    pub object_kind: Option<String>,
    pub object_id: Option<String>,
    pub request_id: Option<Uuid>,
    pub application_id: Option<i32>,
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum AuditActor {
    Anonymous,
    User(i32),
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditFilter {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub actor: Option<AuditActor>,
    pub action: Option<String>,
    pub object_kind: Option<String>,
    pub object_id: Option<String>,
    pub request_id: Option<Uuid>,
    pub application_id: Option<i32>,
    pub result: Option<String>,
}

impl AuditFilterInput {
    pub fn validate(self) -> Result<AuditFilter, IdentityError> {
        fn time(value: String) -> Result<i64, IdentityError> {
            if value.len() > 64 {
                return Err(IdentityError::InvalidInput);
            }
            let millis = chrono::DateTime::parse_from_rfc3339(&value)
                .map_err(|_| IdentityError::InvalidInput)?
                .timestamp_millis();
            if millis < 0 {
                return Err(IdentityError::InvalidInput);
            }
            Ok(millis)
        }
        fn text(value: Option<String>) -> Result<Option<String>, IdentityError> {
            value
                .map(|value| {
                    if value.is_empty()
                        || value.trim() != value
                        || value.chars().count() > 256
                        || value.chars().any(char::is_control)
                    {
                        Err(IdentityError::InvalidInput)
                    } else {
                        Ok(value)
                    }
                })
                .transpose()
        }
        let from = self.from.map(time).transpose()?;
        let to = self.to.map(time).transpose()?;
        if matches!((from,to), (Some(from),Some(to)) if from >= to)
            || self.application_id.is_some_and(|id| id <= 0)
        {
            return Err(IdentityError::InvalidInput);
        }
        let actor = self
            .actor
            .map(|value| {
                if value == "anonymous" {
                    return Ok(AuditActor::Anonymous);
                }
                let id = value
                    .parse::<i32>()
                    .map_err(|_| IdentityError::InvalidInput)?;
                if id <= 0 || id.to_string() != value {
                    return Err(IdentityError::InvalidInput);
                }
                Ok(AuditActor::User(id))
            })
            .transpose()?;
        let result = text(self.result)?;
        if result
            .as_deref()
            .is_some_and(|result| !matches!(result, "success" | "denied" | "failure"))
        {
            return Err(IdentityError::InvalidInput);
        }
        Ok(AuditFilter {
            from,
            to,
            actor,
            action: text(self.action)?,
            object_kind: text(self.object_kind)?,
            object_id: text(self.object_id)?,
            request_id: self.request_id,
            application_id: self.application_id,
            result,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditPosition {
    pub happened_at: i64,
    pub id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditCursor {
    version: u8,
    fingerprint: String,
    pub upper: AuditPosition,
    pub last: AuditPosition,
}
impl AuditCursor {
    fn fingerprint(filter: &AuditFilter) -> Result<String, IdentityError> {
        Ok(hex::encode(Sha256::digest(
            serde_json::to_vec(filter).map_err(|_| IdentityError::InvalidInput)?,
        )))
    }
    pub fn new(
        filter: &AuditFilter,
        upper: AuditPosition,
        last: AuditPosition,
    ) -> Result<Self, IdentityError> {
        let value = Self {
            version: 1,
            fingerprint: Self::fingerprint(filter)?,
            upper,
            last,
        };
        value.validate(filter)?;
        Ok(value)
    }
    fn validate(&self, filter: &AuditFilter) -> Result<(), IdentityError> {
        if self.version != 1
            || self.upper < self.last
            || self.last.happened_at < 0
            || filter.from.is_some_and(|from| self.last.happened_at < from)
            || filter.to.is_some_and(|to| self.upper.happened_at >= to)
        {
            return Err(IdentityError::InvalidInput);
        }
        if self.fingerprint != Self::fingerprint(filter)? {
            return Err(IdentityError::Conflict);
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<String, IdentityError> {
        let encoded = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(self).map_err(|_| IdentityError::InvalidInput)?);
        if encoded.len() > 2048 {
            return Err(IdentityError::InvalidInput);
        }
        Ok(encoded)
    }
    pub fn decode(value: &str, filter: &AuditFilter) -> Result<Self, IdentityError> {
        if value.is_empty() || value.len() > 2048 {
            return Err(IdentityError::InvalidInput);
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| IdentityError::InvalidInput)?;
        let cursor: Self =
            serde_json::from_slice(&decoded).map_err(|_| IdentityError::InvalidInput)?;
        cursor.validate(filter)?;
        Ok(cursor)
    }
}

#[derive(Serialize)]
pub struct AuditPage {
    pub items: Vec<ConsoleAudit>,
    pub next: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_audit_filter() {
        let valid = AuditFilterInput {
            from: Some("2026-11-12T08:00:00+08:00".into()),
            to: Some("2026-11-13T00:00:00Z".into()),
            actor: Some("anonymous".into()),
            result: Some("success".into()),
            ..Default::default()
        }
        .validate()
        .unwrap();
        assert_eq!(valid.actor, Some(AuditActor::Anonymous));
        assert!(valid.from < valid.to);
        for input in [
            AuditFilterInput {
                from: Some("yesterday".into()),
                ..Default::default()
            },
            AuditFilterInput {
                from: Some("2026-11-13T00:00:00Z".into()),
                to: Some("2026-11-12T00:00:00Z".into()),
                ..Default::default()
            },
            AuditFilterInput {
                actor: Some("0".into()),
                ..Default::default()
            },
            AuditFilterInput {
                actor: Some(String::new()),
                ..Default::default()
            },
            AuditFilterInput {
                action: Some("a".repeat(257)),
                ..Default::default()
            },
            AuditFilterInput {
                result: Some("maybe".into()),
                ..Default::default()
            },
            AuditFilterInput {
                application_id: Some(-1),
                ..Default::default()
            },
        ] {
            assert!(input.validate().is_err());
        }
    }

    #[test]
    fn product_audit_cursor() {
        let filter = AuditFilterInput::default().validate().unwrap();
        let upper = AuditPosition {
            happened_at: 2000,
            id: Uuid::from_u128(9),
        };
        let last = AuditPosition {
            happened_at: 1000,
            id: Uuid::from_u128(7),
        };
        let encoded = AuditCursor::new(&filter, upper, last)
            .unwrap()
            .encode()
            .unwrap();
        assert_eq!(AuditCursor::decode(&encoded, &filter).unwrap().last, last);
        let changed = AuditFilterInput {
            actor: Some("5".into()),
            ..Default::default()
        }
        .validate()
        .unwrap();
        assert!(AuditCursor::decode(&encoded, &changed).is_err());
        assert!(AuditCursor::new(&filter, last, upper).is_err());
        for bad in ["!".into(), "a".repeat(2049)] {
            assert!(AuditCursor::decode(&bad, &filter).is_err());
        }
    }
}
