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

    fn gateway_id(&self) -> Option<&str> {
        match self {
            Self::Gateway { gateway_id }
            | Self::Device { gateway_id, .. }
            | Self::Tool { gateway_id, .. }
            | Self::Task { gateway_id, .. }
            | Self::GatewayKey { gateway_id, .. } => Some(gateway_id),
            _ => None,
        }
    }

    fn device_id(&self) -> Option<&str> {
        match self {
            Self::Device { device_id, .. }
            | Self::Tool { device_id, .. }
            | Self::Task { device_id, .. } => Some(device_id),
            _ => None,
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

    /// Scope is explicit: gateway/device scopes match only their declared kind,
    /// never every descendant kind or another application's identical identifiers.
    pub fn matches(&self, resource: &ResourceRef) -> bool {
        if self.application != resource.application {
            return false;
        }
        let target = resource.target();
        match &self.scope {
            ResourceScope::Exact { target: expected } => expected == target,
            ResourceScope::Application { kind } => *kind == target.kind(),
            ResourceScope::Gateway { gateway_id, kind } => {
                *kind == target.kind() && target.gateway_id() == Some(gateway_id.as_str())
            }
            ResourceScope::Device {
                gateway_id,
                device_id,
                kind,
            } => {
                *kind == target.kind()
                    && target.gateway_id() == Some(gateway_id.as_str())
                    && target.device_id() == Some(device_id.as_str())
            }
        }
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

    pub fn allows(&self, operation: &OperationId, resource: &ResourceRef) -> bool {
        self.grants
            .iter()
            .any(|grant| grant.matches(operation, resource))
    }

    /// Preserve only operations and resource scopes allowed by both ceilings.
    pub fn intersection(&self, other: &Self) -> Self {
        let mut grants = Vec::new();
        for left in &self.grants {
            let mut resources = Vec::new();
            for right in other
                .grants
                .iter()
                .filter(|r| r.operation == left.operation)
            {
                for a in &left.resources {
                    for b in &right.resources {
                        if let Some(scope) = a.intersection(b)
                            && !resources.contains(&scope)
                        {
                            resources.push(scope);
                        }
                    }
                }
            }
            if !resources.is_empty() {
                grants.push(ScopedOperation {
                    operation: left.operation.clone(),
                    resources,
                });
            }
        }
        Self::new(grants)
    }
}

impl ResourceSelector {
    /// The overlap of two finite typed scopes can only be the narrower scope.
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        if self.application != other.application {
            return None;
        }
        if let ResourceScope::Exact { target } = &self.scope {
            return other
                .matches(&ResourceRef {
                    application: self.application.clone(),
                    target: target.clone(),
                })
                .then(|| self.clone());
        }
        if let ResourceScope::Exact { .. } = &other.scope {
            return other.intersection(self);
        }
        let kind = |scope: &ResourceScope| match scope {
            ResourceScope::Application { kind }
            | ResourceScope::Gateway { kind, .. }
            | ResourceScope::Device { kind, .. } => *kind,
            ResourceScope::Exact { target } => target.kind(),
        };
        if kind(&self.scope) != kind(&other.scope) {
            return None;
        }
        match (&self.scope, &other.scope) {
            (ResourceScope::Application { .. }, _) => Some(other.clone()),
            (_, ResourceScope::Application { .. }) => Some(self.clone()),
            (
                ResourceScope::Gateway { gateway_id: a, .. },
                ResourceScope::Gateway { gateway_id: b, .. },
            ) => (a == b).then(|| self.clone()),
            (
                ResourceScope::Gateway { gateway_id: a, .. },
                ResourceScope::Device { gateway_id: b, .. },
            ) => (a == b).then(|| other.clone()),
            (ResourceScope::Device { .. }, ResourceScope::Gateway { .. }) => {
                other.intersection(self)
            }
            (
                ResourceScope::Device {
                    gateway_id: a,
                    device_id: x,
                    ..
                },
                ResourceScope::Device {
                    gateway_id: b,
                    device_id: y,
                    ..
                },
            ) => (a == b && x == y).then(|| self.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::*;

    #[test]
    fn security_t26_scope_intersection_keeps_narrower_resources_and_application() {
        let broad = ResourceSelector::new(
            "app",
            ResourceScope::Application {
                kind: ResourceKind::Device,
            },
        )
        .unwrap();
        let gateway = ResourceSelector::new(
            "app",
            ResourceScope::Gateway {
                gateway_id: "g1".into(),
                kind: ResourceKind::Device,
            },
        )
        .unwrap();
        let device = ResourceSelector::new(
            "app",
            ResourceScope::Device {
                gateway_id: "g1".into(),
                device_id: "d1".into(),
                kind: ResourceKind::Device,
            },
        )
        .unwrap();
        let exact = ResourceSelector::new(
            "app",
            ResourceScope::Exact {
                target: ResourceTarget::Device {
                    gateway_id: "g1".into(),
                    device_id: "d1".into(),
                },
            },
        )
        .unwrap();
        for (left, right, expected) in [
            (&broad, &gateway, &gateway),
            (&gateway, &device, &device),
            (&device, &exact, &exact),
        ] {
            assert_eq!(left.intersection(right).as_ref(), Some(expected));
            assert_eq!(right.intersection(left).as_ref(), Some(expected));
        }
        for other in [
            ResourceSelector::new("other", broad.scope.clone()).unwrap(),
            ResourceSelector::new(
                "app",
                ResourceScope::Gateway {
                    gateway_id: "g2".into(),
                    kind: ResourceKind::Device,
                },
            )
            .unwrap(),
            ResourceSelector::new(
                "app",
                ResourceScope::Application {
                    kind: ResourceKind::Tool,
                },
            )
            .unwrap(),
        ] {
            assert!(gateway.intersection(&other).is_none());
        }
    }

    #[test]
    fn security_t26_bound_intersection_excludes_new_operations_and_wider_scopes() {
        let bound = |operation: &str, scope| {
            CredentialBounds::new(vec![
                ScopedOperation::new(
                    OperationId::new(operation).unwrap(),
                    vec![ResourceSelector::new("app", scope).unwrap()],
                )
                .unwrap(),
            ])
        };
        let old = bound(
            "device:read",
            ResourceScope::Gateway {
                gateway_id: "g1".into(),
                kind: ResourceKind::Device,
            },
        );
        let current = bound(
            "device:read",
            ResourceScope::Application {
                kind: ResourceKind::Device,
            },
        );
        let intersection = old.intersection(&current);
        for (gateway, expected) in [("g1", true), ("g2", false)] {
            assert_eq!(
                intersection.allows(
                    &OperationId::new("device:read").unwrap(),
                    &ResourceRef::new(
                        "app",
                        ResourceTarget::Device {
                            gateway_id: gateway.into(),
                            device_id: "d".into()
                        }
                    )
                    .unwrap()
                ),
                expected
            );
        }
        assert!(
            old.intersection(&bound(
                "device:write",
                ResourceScope::Application {
                    kind: ResourceKind::Device
                }
            ))
            .grants()
            .is_empty()
        );
        assert!(
            old.intersection(&CredentialBounds::new(vec![]))
                .grants()
                .is_empty()
        );
    }
}

impl ScopedOperation {
    fn matches(&self, operation: &OperationId, resource: &ResourceRef) -> bool {
        self.operation == *operation && self.resources.iter().any(|scope| scope.matches(resource))
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicySubject {
    Principal(PrincipalRef),
    Role(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleEffect {
    Allow,
    Deny,
}

/// Only trusted resource facts satisfy these constraints. A missing fact is
/// indeterminate, not evidence that a conditional deny does not apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceConstraint {
    OwnerIsActor,
    CapabilityRevision(String),
    TargetBindingRevision(String),
}
impl ResourceConstraint {
    fn validate(&self) -> Result<(), SecurityError> {
        match self {
            Self::OwnerIsActor => Ok(()),
            Self::CapabilityRevision(value) | Self::TargetBindingRevision(value) => {
                validate_component(value)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PolicyRule {
    subject: PolicySubject,
    effect: RuleEffect,
    grant: ScopedOperation,
    constraints: Vec<ResourceConstraint>,
}
impl PolicyRule {
    pub fn new(
        subject: PolicySubject,
        effect: RuleEffect,
        grant: ScopedOperation,
        constraints: Vec<ResourceConstraint>,
    ) -> Result<Self, SecurityError> {
        if let PolicySubject::Role(role) = &subject {
            validate_component(role)?;
        }
        for constraint in &constraints {
            constraint.validate()?;
        }
        Ok(Self {
            subject,
            effect,
            grant,
            constraints,
        })
    }
}

/// Loaded from one consistent trusted snapshot; never deserialized from requests.
/// Credential/root liveness and snapshot freshness must be checked by the caller.
/// Resource adapters supply mandatory constraints (e.g. OwnerIsActor for task:read).
#[derive(Debug, Clone)]
pub struct PolicySnapshot {
    principal: PrincipalRef,
    application: String,
    current_roles: Vec<String>,
    registered_operations: Vec<OperationId>,
    credential_bounds: CredentialBounds,
    rules: Vec<PolicyRule>,
    constraints: Vec<ResourceConstraint>,
}
impl PolicySnapshot {
    /// The initial snapshot has no registered operations or current grants.
    pub fn new(
        principal: PrincipalRef,
        application: impl Into<String>,
        credential_bounds: CredentialBounds,
    ) -> Result<Self, SecurityError> {
        let application = application.into();
        validate_component(&application)?;
        Ok(Self {
            principal,
            application,
            credential_bounds,
            current_roles: vec![],
            registered_operations: vec![],
            rules: vec![],
            constraints: vec![],
        })
    }
    pub fn with_roles(mut self, roles: Vec<String>) -> Result<Self, SecurityError> {
        for role in &roles {
            validate_component(role)?;
        }
        self.current_roles = roles;
        Ok(self)
    }
    pub fn with_registered_operations(mut self, operations: Vec<OperationId>) -> Self {
        self.registered_operations = operations;
        self
    }
    pub fn with_rules(mut self, rules: Vec<PolicyRule>) -> Self {
        self.rules = rules;
        self
    }
    pub fn with_constraints(
        mut self,
        constraints: Vec<ResourceConstraint>,
    ) -> Result<Self, SecurityError> {
        for constraint in &constraints {
            constraint.validate()?;
        }
        self.constraints = constraints;
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAllowReason {
    Principal,
    Role,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDenyReason {
    ApplicationMismatch,
    UnknownOperation,
    CredentialBounds,
    ResourceConstraint,
    MissingResourceFacts,
    ExplicitDeny,
    NoGrant,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyEffect {
    Allow(PolicyAllowReason),
    Deny(PolicyDenyReason),
}

#[derive(Clone, Copy)]
enum ConstraintMatch {
    Match,
    NoMatch,
    MissingFacts,
}
fn match_constraints(
    constraints: &[ResourceConstraint],
    principal: &PrincipalRef,
    facts: &ResourceFacts,
) -> ConstraintMatch {
    let mut missing = false;
    for constraint in constraints {
        let matches = match constraint {
            ResourceConstraint::OwnerIsActor => {
                facts.owner.as_ref().map(|owner| owner == principal)
            }
            ResourceConstraint::CapabilityRevision(expected) => facts
                .capability_revision
                .as_ref()
                .map(|value| value == expected),
            ResourceConstraint::TargetBindingRevision(expected) => facts
                .target_binding_revision
                .as_ref()
                .map(|value| value == expected),
        };
        match matches {
            Some(false) => return ConstraintMatch::NoMatch,
            Some(true) => {}
            None => missing = true,
        }
    }
    if missing {
        ConstraintMatch::MissingFacts
    } else {
        ConstraintMatch::Match
    }
}

/// Pure RBAC/ABAC intersection. There is deliberately no super-admin bypass.
/// Dependency errors are handled before constructing a snapshot, not converted
/// into policy effects here. All matching denies are examined before any allow.
pub fn evaluate_policy(snapshot: &PolicySnapshot, operation: &OperationContext) -> PolicyEffect {
    use PolicyDenyReason as Deny;
    if snapshot.application != operation.resource.application {
        return PolicyEffect::Deny(Deny::ApplicationMismatch);
    }
    if !snapshot
        .registered_operations
        .contains(&operation.operation)
    {
        return PolicyEffect::Deny(Deny::UnknownOperation);
    }
    if !snapshot
        .credential_bounds
        .allows(&operation.operation, &operation.resource)
    {
        return PolicyEffect::Deny(Deny::CredentialBounds);
    }
    match match_constraints(&snapshot.constraints, &snapshot.principal, &operation.facts) {
        ConstraintMatch::NoMatch => return PolicyEffect::Deny(Deny::ResourceConstraint),
        ConstraintMatch::MissingFacts => return PolicyEffect::Deny(Deny::MissingResourceFacts),
        ConstraintMatch::Match => {}
    }
    let mut principal_allow = false;
    let mut role_allow = false;
    let mut missing_deny_facts = false;
    let mut missing_allow_facts = false;
    for rule in &snapshot.rules {
        let is_principal = match &rule.subject {
            PolicySubject::Principal(principal) if principal == &snapshot.principal => true,
            PolicySubject::Role(role) if snapshot.current_roles.contains(role) => false,
            _ => continue,
        };
        if !rule
            .grant
            .matches(&operation.operation, &operation.resource)
        {
            continue;
        }
        match match_constraints(&rule.constraints, &snapshot.principal, &operation.facts) {
            ConstraintMatch::NoMatch => {}
            ConstraintMatch::MissingFacts => match rule.effect {
                RuleEffect::Deny => missing_deny_facts = true,
                RuleEffect::Allow => missing_allow_facts = true,
            },
            ConstraintMatch::Match => match rule.effect {
                RuleEffect::Deny => return PolicyEffect::Deny(Deny::ExplicitDeny),
                RuleEffect::Allow if is_principal => principal_allow = true,
                RuleEffect::Allow => role_allow = true,
            },
        }
    }
    if missing_deny_facts {
        PolicyEffect::Deny(Deny::MissingResourceFacts)
    } else if principal_allow {
        PolicyEffect::Allow(PolicyAllowReason::Principal)
    } else if role_allow {
        PolicyEffect::Allow(PolicyAllowReason::Role)
    } else if missing_allow_facts {
        PolicyEffect::Deny(Deny::MissingResourceFacts)
    } else {
        PolicyEffect::Deny(Deny::NoGrant)
    }
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

    fn actor(subject: &str) -> PrincipalRef {
        PrincipalRef::new(
            "https://idp.test",
            super::super::identity::PrincipalKind::User,
            subject,
        )
        .unwrap()
    }
    fn device(application: &str, gateway: &str, id: &str) -> ResourceRef {
        ResourceRef::new(
            application,
            ResourceTarget::Device {
                gateway_id: gateway.into(),
                device_id: id.into(),
            },
        )
        .unwrap()
    }
    fn grant(operation: &str, resource: &ResourceRef) -> ScopedOperation {
        ScopedOperation::new(
            OperationId::new(operation).unwrap(),
            vec![
                ResourceSelector::new(
                    resource.application(),
                    ResourceScope::Exact {
                        target: resource.target().clone(),
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }
    fn rule(subject: PolicySubject, effect: RuleEffect, grant: ScopedOperation) -> PolicyRule {
        PolicyRule::new(subject, effect, grant, vec![]).unwrap()
    }
    fn snapshot(resource: &ResourceRef, operation: &str) -> PolicySnapshot {
        PolicySnapshot::new(
            actor("alice"),
            resource.application(),
            CredentialBounds::new(vec![grant(operation, resource)]),
        )
        .unwrap()
        .with_registered_operations(vec![OperationId::new(operation).unwrap()])
        .with_roles(vec!["operator".into(), "super_admin".into()])
        .unwrap()
    }
    fn context(resource: ResourceRef, operation: &str) -> OperationContext {
        OperationContext {
            operation: OperationId::new(operation).unwrap(),
            resource,
            facts: ResourceFacts::default(),
        }
    }

    #[test]
    fn security_t11_every_matching_deny_precedes_user_and_super_admin_allows() {
        let resource = device("app", "g", "d");
        let user = PolicySubject::Principal(actor("alice"));
        let role = PolicySubject::Role("super_admin".into());
        let g = grant("device:read", &resource);
        for denied in [user.clone(), role.clone()] {
            let rules = vec![
                rule(user.clone(), RuleEffect::Allow, g.clone()),
                rule(role.clone(), RuleEffect::Allow, g.clone()),
                rule(denied, RuleEffect::Deny, g.clone()),
            ];
            for offset in 0..rules.len() {
                let mut reordered = rules.clone();
                reordered.rotate_left(offset);
                let s = snapshot(&resource, "device:read").with_rules(reordered);
                assert_eq!(
                    evaluate_policy(&s, &context(resource.clone(), "device:read")),
                    PolicyEffect::Deny(PolicyDenyReason::ExplicitDeny)
                );
            }
        }
    }

    #[test]
    fn security_t11_user_allow_role_allow_and_default_deny_have_distinct_precedence() {
        let resource = device("app", "g", "d");
        let g = grant("device:read", &resource);
        let mut s = snapshot(&resource, "device:read").with_rules(vec![rule(
            PolicySubject::Role("operator".into()),
            RuleEffect::Allow,
            g.clone(),
        )]);
        let ctx = context(resource.clone(), "device:read");
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Allow(PolicyAllowReason::Role)
        );
        s.rules.push(rule(
            PolicySubject::Principal(actor("alice")),
            RuleEffect::Allow,
            g,
        ));
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Allow(PolicyAllowReason::Principal)
        );
        s.rules.clear();
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::NoGrant)
        );
    }

    #[test]
    fn security_t11_unrelated_denies_do_not_override_a_valid_grant() {
        let resource = device("app", "g", "d");
        let g = grant("device:read", &resource);
        let rules = vec![
            rule(
                PolicySubject::Principal(actor("alice")),
                RuleEffect::Allow,
                g.clone(),
            ),
            rule(
                PolicySubject::Principal(actor("bob")),
                RuleEffect::Deny,
                g.clone(),
            ),
            rule(
                PolicySubject::Role("not_assigned".into()),
                RuleEffect::Deny,
                g,
            ),
            rule(
                PolicySubject::Role("operator".into()),
                RuleEffect::Deny,
                grant("device:write", &resource),
            ),
            rule(
                PolicySubject::Role("operator".into()),
                RuleEffect::Deny,
                grant("device:read", &device("app", "g", "other")),
            ),
            rule(
                PolicySubject::Role("operator".into()),
                RuleEffect::Deny,
                grant("device:read", &device("other-app", "g", "d")),
            ),
        ];
        assert_eq!(
            evaluate_policy(
                &snapshot(&resource, "device:read").with_rules(rules),
                &context(resource.clone(), "device:read")
            ),
            PolicyEffect::Allow(PolicyAllowReason::Principal)
        );
    }

    #[test]
    fn security_t11_broad_deny_beats_narrow_user_allow_in_any_rule_order() {
        let resource = device("app", "g", "d");
        let broad = ScopedOperation::new(
            OperationId::new("device:read").unwrap(),
            vec![
                ResourceSelector::new(
                    "app",
                    ResourceScope::Gateway {
                        gateway_id: "g".into(),
                        kind: ResourceKind::Device,
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let rules = vec![
            rule(
                PolicySubject::Principal(actor("alice")),
                RuleEffect::Allow,
                grant("device:read", &resource),
            ),
            rule(
                PolicySubject::Role("operator".into()),
                RuleEffect::Deny,
                broad,
            ),
        ];
        for offset in 0..rules.len() {
            let mut reordered = rules.clone();
            reordered.rotate_left(offset);
            assert_eq!(
                evaluate_policy(
                    &snapshot(&resource, "device:read").with_rules(reordered),
                    &context(resource.clone(), "device:read")
                ),
                PolicyEffect::Deny(PolicyDenyReason::ExplicitDeny)
            );
        }
    }

    #[test]
    fn security_t12_current_roles_and_signed_bounds_are_both_required() {
        let resource = device("app", "g", "d");
        let ctx = context(resource.clone(), "device:write");
        let rules = vec![rule(
            PolicySubject::Role("operator".into()),
            RuleEffect::Allow,
            grant("device:write", &resource),
        )];
        let old = snapshot(&resource, "device:read")
            .with_registered_operations(vec![ctx.operation.clone()])
            .with_rules(rules.clone());
        assert_eq!(
            evaluate_policy(&old, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::CredentialBounds)
        );
        let fresh = snapshot(&resource, "device:write").with_rules(rules);
        assert_eq!(
            evaluate_policy(&fresh, &ctx),
            PolicyEffect::Allow(PolicyAllowReason::Role)
        );
        let revoked = fresh.with_roles(vec![]).unwrap();
        assert_eq!(
            evaluate_policy(&revoked, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::NoGrant)
        );
        let direct = revoked.with_rules(vec![rule(
            PolicySubject::Principal(actor("alice")),
            RuleEffect::Allow,
            ScopedOperation::new(
                ctx.operation.clone(),
                vec![
                    ResourceSelector::new(
                        "app",
                        ResourceScope::Application {
                            kind: ResourceKind::Device,
                        },
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        )]);
        assert_eq!(
            evaluate_policy(&direct, &ctx),
            PolicyEffect::Allow(PolicyAllowReason::Principal)
        );
        for out_of_bounds in [
            device("app", "other-gateway", "d"),
            device("app", "g", "other-device"),
        ] {
            assert_eq!(
                evaluate_policy(&direct, &context(out_of_bounds, "device:write")),
                PolicyEffect::Deny(PolicyDenyReason::CredentialBounds)
            );
        }
        let empty = PolicySnapshot::new(actor("alice"), "app", CredentialBounds::new(vec![]))
            .unwrap()
            .with_registered_operations(vec![ctx.operation.clone()])
            .with_rules(vec![rule(
                PolicySubject::Principal(actor("alice")),
                RuleEffect::Allow,
                grant("device:write", &resource),
            )]);
        assert_eq!(
            evaluate_policy(&empty, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::CredentialBounds)
        );
    }

    #[test]
    fn security_t12_scope_matching_never_crosses_application_gateway_device_or_kind() {
        let target = device("app", "g", "d");
        for scope in [
            ResourceScope::Exact {
                target: target.target().clone(),
            },
            ResourceScope::Application {
                kind: ResourceKind::Device,
            },
            ResourceScope::Gateway {
                gateway_id: "g".into(),
                kind: ResourceKind::Device,
            },
            ResourceScope::Device {
                gateway_id: "g".into(),
                device_id: "d".into(),
                kind: ResourceKind::Device,
            },
        ] {
            let selector = ResourceSelector::new("app", scope.clone()).unwrap();
            assert!(selector.matches(&target));
            assert!(!selector.matches(&device("other", "g", "d")));
            assert!(
                !selector.matches(
                    &ResourceRef::new(
                        "app",
                        ResourceTarget::Tool {
                            gateway_id: "g".into(),
                            device_id: "d".into(),
                            tool_name: "t".into(),
                        }
                    )
                    .unwrap()
                )
            );
            if !matches!(scope, ResourceScope::Application { .. }) {
                assert!(!selector.matches(&device("app", "other", "d")));
            }
            if matches!(
                scope,
                ResourceScope::Exact { .. } | ResourceScope::Device { .. }
            ) {
                assert!(!selector.matches(&device("app", "g", "other")));
            }
        }
        let s = snapshot(&target, "device:read").with_rules(vec![rule(
            PolicySubject::Role("super_admin".into()),
            RuleEffect::Allow,
            grant("device:read", &target),
        )]);
        assert_eq!(
            evaluate_policy(&s, &context(device("other", "g", "d"), "device:read")),
            PolicyEffect::Deny(PolicyDenyReason::ApplicationMismatch)
        );
        let unknown = snapshot(&target, "device:read")
            .with_registered_operations(vec![])
            .with_rules(s.rules);
        assert_eq!(
            evaluate_policy(&unknown, &context(target, "device:read")),
            PolicyEffect::Deny(PolicyDenyReason::UnknownOperation)
        );
    }

    #[test]
    fn security_t12_owner_and_version_constraints_cannot_be_bypassed_by_role_allow() {
        let resource = ResourceRef::new(
            "app",
            ResourceTarget::Task {
                gateway_id: "g".into(),
                device_id: "d".into(),
                task_id: "task".into(),
            },
        )
        .unwrap();
        let s = snapshot(&resource, "task:read")
            .with_rules(vec![rule(
                PolicySubject::Role("super_admin".into()),
                RuleEffect::Allow,
                grant("task:read", &resource),
            )])
            .with_constraints(vec![
                ResourceConstraint::OwnerIsActor,
                ResourceConstraint::CapabilityRevision("cap1".into()),
                ResourceConstraint::TargetBindingRevision("target1".into()),
            ])
            .unwrap();
        let mut ctx = context(resource, "task:read");
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::MissingResourceFacts)
        );
        ctx.facts = ResourceFacts {
            owner: Some(actor("alice")),
            capability_revision: Some("cap1".into()),
            target_binding_revision: Some("target1".into()),
        };
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Allow(PolicyAllowReason::Role)
        );
        ctx.facts.owner = Some(actor("bob"));
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::ResourceConstraint)
        );
        ctx.facts.owner = Some(actor("alice"));
        ctx.facts.target_binding_revision = Some("target2".into());
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::ResourceConstraint)
        );
    }

    #[test]
    fn security_t12_missing_deny_facts_fail_closed_and_missing_allow_facts_do_not_grant() {
        let resource = device("app", "g", "d");
        let conditional = PolicyRule::new(
            PolicySubject::Role("operator".into()),
            RuleEffect::Deny,
            grant("device:read", &resource),
            vec![ResourceConstraint::OwnerIsActor],
        )
        .unwrap();
        let allow = rule(
            PolicySubject::Principal(actor("alice")),
            RuleEffect::Allow,
            grant("device:read", &resource),
        );
        let s = snapshot(&resource, "device:read").with_rules(vec![allow, conditional]);
        let mut ctx = context(resource.clone(), "device:read");
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::MissingResourceFacts)
        );
        ctx.facts.owner = Some(actor("bob"));
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Allow(PolicyAllowReason::Principal)
        );
        ctx.facts.owner = Some(actor("alice"));
        assert_eq!(
            evaluate_policy(&s, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::ExplicitDeny)
        );
        let missing_allow = snapshot(&resource, "device:read").with_rules(vec![
            PolicyRule::new(
                PolicySubject::Principal(actor("alice")),
                RuleEffect::Allow,
                grant("device:read", &resource),
                vec![ResourceConstraint::OwnerIsActor],
            )
            .unwrap(),
        ]);
        ctx.facts.owner = None;
        assert_eq!(
            evaluate_policy(&missing_allow, &ctx),
            PolicyEffect::Deny(PolicyDenyReason::MissingResourceFacts)
        );
        assert!(
            PolicyRule::new(
                PolicySubject::Role("".into()),
                RuleEffect::Allow,
                grant("device:read", &resource),
                vec![]
            )
            .is_err()
        );
        assert!(
            snapshot(&resource, "device:read")
                .with_constraints(vec![ResourceConstraint::CapabilityRevision("".into())])
                .is_err()
        );
    }

    #[test]
    fn security_t12_principal_and_owner_matching_uses_full_identity_namespace() {
        use super::super::identity::PrincipalKind;
        let resource = device("app", "g", "d");
        for impostor in [
            PrincipalRef::new("https://other.test", PrincipalKind::User, "alice").unwrap(),
            PrincipalRef::new("https://idp.test", PrincipalKind::Machine, "alice").unwrap(),
        ] {
            let s = snapshot(&resource, "device:read").with_rules(vec![rule(
                PolicySubject::Principal(impostor.clone()),
                RuleEffect::Allow,
                grant("device:read", &resource),
            )]);
            let mut ctx = context(resource.clone(), "device:read");
            assert_eq!(
                evaluate_policy(&s, &ctx),
                PolicyEffect::Deny(PolicyDenyReason::NoGrant)
            );
            let constrained = s
                .with_rules(vec![rule(
                    PolicySubject::Role("operator".into()),
                    RuleEffect::Allow,
                    grant("device:read", &resource),
                )])
                .with_constraints(vec![ResourceConstraint::OwnerIsActor])
                .unwrap();
            ctx.facts.owner = Some(impostor);
            assert_eq!(
                evaluate_policy(&constrained, &ctx),
                PolicyEffect::Deny(PolicyDenyReason::ResourceConstraint)
            );
        }
    }

    #[test]
    fn security_t12_gateway_and_device_descendant_scopes_remain_typed_and_exact() {
        for target in [
            ResourceTarget::Gateway {
                gateway_id: "g".into(),
            },
            ResourceTarget::Tool {
                gateway_id: "g".into(),
                device_id: "d".into(),
                tool_name: "t".into(),
            },
            ResourceTarget::Task {
                gateway_id: "g".into(),
                device_id: "d".into(),
                task_id: "id".into(),
            },
            ResourceTarget::GatewayKey {
                gateway_id: "g".into(),
                kid: "key".into(),
            },
        ] {
            let resource = ResourceRef::new("app", target.clone()).unwrap();
            let gateway_scope = ResourceSelector::new(
                "app",
                ResourceScope::Gateway {
                    gateway_id: "g".into(),
                    kind: target.kind(),
                },
            )
            .unwrap();
            assert!(gateway_scope.matches(&resource));
            let other_gateway = ResourceSelector::new(
                "app",
                ResourceScope::Gateway {
                    gateway_id: "g2".into(),
                    kind: target.kind(),
                },
            )
            .unwrap();
            assert!(!other_gateway.matches(&resource));
            if matches!(target.kind(), ResourceKind::Tool | ResourceKind::Task) {
                for (id, expected) in [("d", true), ("d2", false)] {
                    let scope = ResourceSelector::new(
                        "app",
                        ResourceScope::Device {
                            gateway_id: "g".into(),
                            device_id: id.into(),
                            kind: target.kind(),
                        },
                    )
                    .unwrap();
                    assert_eq!(scope.matches(&resource), expected);
                }
            }
        }
        for target in [
            ResourceTarget::Application,
            ResourceTarget::IdpRoute {
                route_id: "r".into(),
            },
            ResourceTarget::Role {
                role_id: "r".into(),
            },
            ResourceTarget::Permission {
                permission_id: "p".into(),
            },
            ResourceTarget::ServiceBinding {
                client_id: "c".into(),
            },
        ] {
            let selector = ResourceSelector::new(
                "app",
                ResourceScope::Exact {
                    target: target.clone(),
                },
            )
            .unwrap();
            let resource = ResourceRef::new("app", target).unwrap();
            assert!(selector.matches(&resource));
            assert!(!selector.matches(&device("app", "g", "d")));
        }
    }
}
