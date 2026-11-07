use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use super::{error::SecurityError, identity::PrincipalRef};

fn validate_component(value: &str) -> Result<(), SecurityError> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(SecurityError::InvalidResource);
    }
    Ok(())
}

/// Syntax validation is separate from the server's registered operation allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OperationId(String);

impl OperationId {
    pub fn new(value: impl Into<String>) -> Result<Self, SecurityError> {
        let value = value.into();
        if value.len() > 128
            || !value.contains(':')
            || value.split(':').any(|part| {
                part.is_empty()
                    || !part.bytes().all(|c| {
                        c.is_ascii_lowercase()
                            || c.is_ascii_digit()
                            || matches!(c, b'_' | b'-' | b'.')
                    })
            })
        {
            return Err(SecurityError::InvalidResource);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OperationId {
    type Error = SecurityError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<OperationId> for String {
    fn from(value: OperationId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Application,
    Gateway,
    Device,
    Tool,
    Task,
    IdpRoute,
    Role,
    Permission,
    ServiceBinding,
    GatewayKey,
}

/// Variants make missing fields unrepresentable; validation is enforced by ResourceRef.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceTarget {
    Application,
    Gateway {
        gateway_id: String,
    },
    Device {
        gateway_id: String,
        device_id: String,
    },
    Tool {
        gateway_id: String,
        device_id: String,
        tool_name: String,
    },
    Task {
        gateway_id: String,
        device_id: String,
        task_id: String,
    },
    IdpRoute {
        route_id: String,
    },
    Role {
        role_id: String,
    },
    Permission {
        permission_id: String,
    },
    ServiceBinding {
        client_id: String,
    },
    GatewayKey {
        gateway_id: String,
        kid: String,
    },
}

impl ResourceTarget {
    pub fn kind(&self) -> ResourceKind {
        match self {
            Self::Application => ResourceKind::Application,
            Self::Gateway { .. } => ResourceKind::Gateway,
            Self::Device { .. } => ResourceKind::Device,
            Self::Tool { .. } => ResourceKind::Tool,
            Self::Task { .. } => ResourceKind::Task,
            Self::IdpRoute { .. } => ResourceKind::IdpRoute,
            Self::Role { .. } => ResourceKind::Role,
            Self::Permission { .. } => ResourceKind::Permission,
            Self::ServiceBinding { .. } => ResourceKind::ServiceBinding,
            Self::GatewayKey { .. } => ResourceKind::GatewayKey,
        }
    }

    fn validate(&self) -> Result<(), SecurityError> {
        match self {
            Self::Application => Ok(()),
            Self::Gateway { gateway_id } => validate_component(gateway_id),
            Self::Device {
                gateway_id,
                device_id,
            } => {
                validate_component(gateway_id)?;
                validate_component(device_id)
            }
            Self::Tool {
                gateway_id,
                device_id,
                tool_name,
            } => {
                validate_component(gateway_id)?;
                validate_component(device_id)?;
                validate_component(tool_name)
            }
            Self::Task {
                gateway_id,
                device_id,
                task_id,
            } => {
                validate_component(gateway_id)?;
                validate_component(device_id)?;
                validate_component(task_id)
            }
            Self::IdpRoute { route_id } => validate_component(route_id),
            Self::Role { role_id } => validate_component(role_id),
            Self::Permission { permission_id } => validate_component(permission_id),
            Self::ServiceBinding { client_id } => validate_component(client_id),
            Self::GatewayKey { gateway_id, kid } => {
                validate_component(gateway_id)?;
                validate_component(kid)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "ResourceWire")]
pub struct ResourceRef {
    application: String,
    target: ResourceTarget,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceWire {
    application: String,
    target: ResourceTarget,
}

impl ResourceRef {
    pub fn new(
        application: impl Into<String>,
        target: ResourceTarget,
    ) -> Result<Self, SecurityError> {
        let application = application.into();
        validate_component(&application)?;
        target.validate()?;
        Ok(Self {
            application,
            target,
        })
    }
    pub fn application(&self) -> &str {
        &self.application
    }
    pub fn target(&self) -> &ResourceTarget {
        &self.target
    }
    /// V1 encodes the complete structured tuple, never a delimiter-concatenated alias.
    pub fn canonical_key(&self) -> Result<String, SecurityError> {
        let bytes = serde_json::to_vec(self).map_err(|_| SecurityError::InvalidResource)?;
        Ok(format!("resource:v1:{}", URL_SAFE_NO_PAD.encode(bytes)))
    }
}
impl TryFrom<ResourceWire> for ResourceRef {
    type Error = SecurityError;
    fn try_from(raw: ResourceWire) -> Result<Self, Self::Error> {
        Self::new(raw.application, raw.target)
    }
}

/// Broad scopes must be selected explicitly and always carry an application and kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceScope {
    Exact {
        target: ResourceTarget,
    },
    Application {
        kind: ResourceKind,
    },
    Gateway {
        gateway_id: String,
        kind: ResourceKind,
    },
    Device {
        gateway_id: String,
        device_id: String,
        kind: ResourceKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "SelectorWire")]
pub struct ResourceSelector {
    application: String,
    scope: ResourceScope,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectorWire {
    application: String,
    scope: ResourceScope,
}

impl ResourceSelector {
    pub fn new(
        application: impl Into<String>,
        scope: ResourceScope,
    ) -> Result<Self, SecurityError> {
        let application = application.into();
        validate_component(&application)?;
        match &scope {
            ResourceScope::Exact { target } => target.validate()?,
            ResourceScope::Application { .. } => {}
            ResourceScope::Gateway { gateway_id, kind } => {
                validate_component(gateway_id)?;
                if !matches!(
                    kind,
                    ResourceKind::Gateway
                        | ResourceKind::Device
                        | ResourceKind::Tool
                        | ResourceKind::Task
                        | ResourceKind::GatewayKey
                ) {
                    return Err(SecurityError::InvalidResource);
                }
            }
            ResourceScope::Device {
                gateway_id,
                device_id,
                kind,
            } => {
                validate_component(gateway_id)?;
                validate_component(device_id)?;
                if !matches!(
                    kind,
                    ResourceKind::Device | ResourceKind::Tool | ResourceKind::Task
                ) {
                    return Err(SecurityError::InvalidResource);
                }
            }
        }
        Ok(Self { application, scope })
    }
    pub fn application(&self) -> &str {
        &self.application
    }
    pub fn scope(&self) -> &ResourceScope {
        &self.scope
    }
}
impl TryFrom<SelectorWire> for ResourceSelector {
    type Error = SecurityError;
    fn try_from(raw: SelectorWire) -> Result<Self, Self::Error> {
        Self::new(raw.application, raw.scope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ScopedOperationWire")]
pub struct ScopedOperation {
    operation: OperationId,
    resources: Vec<ResourceSelector>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopedOperationWire {
    operation: OperationId,
    resources: Vec<ResourceSelector>,
}
impl ScopedOperation {
    pub fn new(
        operation: OperationId,
        resources: Vec<ResourceSelector>,
    ) -> Result<Self, SecurityError> {
        if resources.is_empty() {
            return Err(SecurityError::InvalidResource);
        }
        Ok(Self {
            operation,
            resources,
        })
    }
    pub fn operation(&self) -> &OperationId {
        &self.operation
    }
    pub fn resources(&self) -> &[ResourceSelector] {
        &self.resources
    }
}
impl TryFrom<ScopedOperationWire> for ScopedOperation {
    type Error = SecurityError;
    fn try_from(raw: ScopedOperationWire) -> Result<Self, Self::Error> {
        Self::new(raw.operation, raw.resources)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialBounds {
    grants: Vec<ScopedOperation>,
}
impl CredentialBounds {
    /// An empty bound is deny-all, never an implicit wildcard.
    pub fn new(grants: Vec<ScopedOperation>) -> Self {
        Self { grants }
    }
    pub fn grants(&self) -> &[ScopedOperation] {
        &self.grants
    }
}

/// Server-loaded facts, deliberately not deserializable from a client request.
#[derive(Debug, Clone, Default)]
pub struct ResourceFacts {
    pub owner: Option<PrincipalRef>,
    pub capability_revision: Option<String>,
    pub target_binding_revision: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OperationContext {
    pub operation: OperationId,
    pub resource: ResourceRef,
    pub facts: ResourceFacts,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_t04_missing_or_extra_resource_fields_are_rejected() {
        for target in [
            serde_json::json!({"kind":"device","gateway_id":"g"}),
            serde_json::json!({"kind":"gateway","gateway_id":"g","device_id":"d"}),
            serde_json::json!({"kind":"device","gateway_id":"g","device_id":""}),
        ] {
            let raw = serde_json::json!({"application":"app","target":target});
            assert!(serde_json::from_value::<ResourceRef>(raw).is_err());
        }
    }

    #[test]
    fn security_t04_resource_encoding_cannot_merge_separators() {
        let a = ResourceRef::new(
            "app",
            ResourceTarget::Device {
                gateway_id: "a:b".into(),
                device_id: "c".into(),
            },
        )
        .unwrap();
        let b = ResourceRef::new(
            "app",
            ResourceTarget::Device {
                gateway_id: "a".into(),
                device_id: "b:c".into(),
            },
        )
        .unwrap();
        assert_ne!(a.canonical_key(), b.canonical_key());
        let roundtrip: ResourceRef =
            serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
        assert_eq!(roundtrip.canonical_key(), a.canonical_key());
        assert!(a.canonical_key().unwrap().starts_with("resource:v1:"));
    }

    #[test]
    fn security_t04_scope_is_explicit_and_application_bound() {
        let operation = OperationId::new("task:execute").unwrap();
        assert!(OperationId::new("").is_err());
        assert!(OperationId::new("task:*").is_err());
        let selector = ResourceSelector::new(
            "app",
            ResourceScope::Gateway {
                gateway_id: "g".into(),
                kind: ResourceKind::Device,
            },
        )
        .unwrap();
        let bounds = CredentialBounds::new(vec![
            ScopedOperation::new(operation, vec![selector]).unwrap(),
        ]);
        assert_eq!(bounds.grants().len(), 1);
        assert!(
            ResourceSelector::new(
                "",
                ResourceScope::Application {
                    kind: ResourceKind::Device
                }
            )
            .is_err()
        );
        assert!(CredentialBounds::new(vec![]).grants().is_empty());
    }
}
