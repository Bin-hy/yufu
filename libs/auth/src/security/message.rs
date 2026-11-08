//! Application-layer command and gateway event contracts.

use super::{
    budget::{Clock, healthy_reading},
    credential::{CLOCK_ERROR_MILLIS, CredentialRef, DecisionValidity},
    error::SecurityError,
    identity::{PrincipalKind, PrincipalRef, SecretToken, TokenUse},
    policy::{OperationId, ResourceRef, ResourceTarget},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{DeserializeOwned, Error as _},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt, sync::Arc};
use uuid::Uuid;

const MAX_PAYLOAD: usize = 1024 * 1024;
const MAX_SIGNED: usize = 2 * 1024 * 1024;
const PERMIT_TYPE: &str = "cobotrix-operation+jwt";
const EVENT_TYPE: &str = "cobotrix-gateway-message+jwt";
fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RequestDigest([u8; 32]);
impl RequestDigest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }
}
impl TryFrom<String> for RequestDigest {
    type Error = SecurityError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() != 43 {
            return Err(SecurityError::InvalidCredential);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| SecurityError::InvalidCredential)?;
        Ok(Self(
            bytes
                .try_into()
                .map_err(|_| SecurityError::InvalidCredential)?,
        ))
    }
}
impl From<RequestDigest> for String {
    fn from(value: RequestDigest) -> String {
        URL_SAFE_NO_PAD.encode(value.0)
    }
}

/// Explicit wire serialization is allowed, but Debug never exposes signed credentials.
#[derive(Clone)]
pub struct SignedDocument(Arc<SecretToken>);
impl SignedDocument {
    pub fn new(value: String) -> Result<Self, SecurityError> {
        if value.len() > MAX_SIGNED {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self(Arc::new(SecretToken::new(value)?)))
    }
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}
impl fmt::Debug for SignedDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignedDocument([REDACTED])")
    }
}
impl Serialize for SignedDocument {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.expose_secret())
    }
}
impl<'de> Deserialize<'de> for SignedDocument {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PayloadBytes(Vec<u8>);
impl PayloadBytes {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, SecurityError> {
        if bytes.is_empty() || bytes.len() > MAX_PAYLOAD {
            return Err(SecurityError::InvalidResource);
        }
        Ok(Self(bytes))
    }
    pub fn from_payload(payload: &OperationPayload) -> Result<Self, SecurityError> {
        payload.validate()?;
        Self::from_bytes(serde_json::to_vec(payload).map_err(|_| SecurityError::InvalidResource)?)
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    pub fn digest(&self) -> RequestDigest {
        RequestDigest::of(&self.0)
    }
}
impl fmt::Debug for PayloadBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PayloadBytes")
            .field("length", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl TryFrom<String> for PayloadBytes {
    type Error = SecurityError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        if raw.len() > MAX_PAYLOAD.div_ceil(3) * 4 {
            return Err(SecurityError::InvalidResource);
        }
        Self::from_bytes(
            URL_SAFE_NO_PAD
                .decode(raw)
                .map_err(|_| SecurityError::InvalidResource)?,
        )
    }
}
impl From<PayloadBytes> for String {
    fn from(value: PayloadBytes) -> String {
        URL_SAFE_NO_PAD.encode(value.0)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationPayload {
    pub version: u8,
    pub request_id: Uuid,
    pub task_id: Option<Uuid>,
    pub operation: OperationId,
    pub resource: ResourceRef,
    pub tool_name: Option<String>,
    pub capability_revision: Option<String>,
    pub target_binding_revision: Option<String>,
    pub input: serde_json::Value,
    pub headers: BTreeMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub http_method: Option<String>,
    pub route_id: Option<String>,
}
impl OperationPayload {
    fn validate(&self) -> Result<(), SecurityError> {
        if self.version != 1
            || self.request_id.is_nil()
            || self.task_id.is_some_and(|v| v.is_nil())
            || self.timeout_ms == Some(0)
            || self.tool_name.as_ref().is_some_and(|s| !valid_id(s))
            || self
                .capability_revision
                .as_ref()
                .is_some_and(|s| !valid_id(s))
            || self
                .target_binding_revision
                .as_ref()
                .is_some_and(|s| !valid_id(s))
        {
            return Err(SecurityError::InvalidResource);
        }
        if self.operation.as_str() == "task:execute"
            && (self.task_id.is_none()
                || self.tool_name.is_none()
                || self.capability_revision.is_none()
                || self.target_binding_revision.is_none()
                || self.timeout_ms.is_none())
        {
            return Err(SecurityError::InvalidResource);
        }
        if let ResourceTarget::Tool { tool_name, .. } = self.resource.target()
            && self.tool_name.as_ref() != Some(tool_name)
        {
            return Err(SecurityError::InvalidResource);
        }
        if let ResourceTarget::Task { task_id, .. } = self.resource.target()
            && self.task_id.map(|v| v.to_string()).as_ref() != Some(task_id)
        {
            return Err(SecurityError::InvalidResource);
        }
        if self
            .http_method
            .as_ref()
            .is_some_and(|v| http::Method::from_bytes(v.as_bytes()).is_err())
            || self.route_id.as_ref().is_some_and(|v| !valid_id(v))
        {
            return Err(SecurityError::InvalidResource);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayOperationPermit {
    pub iss: String,
    pub aud: String,
    pub jti: String,
    pub iat: u64,
    pub exp: u64,
    pub token_use: TokenUse,
    pub actor: PrincipalRef,
    pub presenter_service: PrincipalRef,
    pub parent_credential: CredentialRef,
    pub grant_id: String,
    pub application: String,
    pub request_id: Uuid,
    pub task_id: Option<Uuid>,
    pub operation: OperationId,
    pub resource: ResourceRef,
    pub request_digest: RequestDigest,
    pub capability_revision: Option<String>,
    pub target_binding_revision: Option<String>,
    pub authorization: DecisionValidity,
}
fn resource_gateway(resource: &ResourceRef) -> Option<&str> {
    match resource.target() {
        ResourceTarget::Gateway { gateway_id }
        | ResourceTarget::Device { gateway_id, .. }
        | ResourceTarget::Tool { gateway_id, .. }
        | ResourceTarget::Task { gateway_id, .. }
        | ResourceTarget::GatewayKey { gateway_id, .. } => Some(gateway_id),
        _ => None,
    }
}
impl GatewayOperationPermit {
    fn validate(&self) -> Result<(), SecurityError> {
        let gateway = resource_gateway(&self.resource).ok_or(SecurityError::InvalidResource)?;
        let expires = self
            .exp
            .checked_mul(1000)
            .ok_or(SecurityError::InvalidCredential)?;
        let issued = self
            .iat
            .checked_mul(1000)
            .ok_or(SecurityError::InvalidCredential)?;
        if self.token_use != TokenUse::GatewayOperation
            || !valid_id(&self.jti)
            || !valid_id(&self.grant_id)
            || self.actor.issuer() != self.iss
            || self.parent_credential.issuer() != self.iss
            || self.presenter_service.issuer() != self.iss
            || self.actor.kind() == PrincipalKind::Device
            || self.presenter_service.kind() != PrincipalKind::Machine
            || self.application != self.resource.application()
            || self.aud != format!("cobotlet:{gateway}")
            || self.exp <= self.iat
            || issued.saturating_add(999) < self.authorization.checked_at()
            || expires > self.authorization.valid_until()
            || self.request_id.is_nil()
            || self.task_id.is_some_and(|v| v.is_nil())
            || (self.operation.as_str() == "task:execute" && self.task_id.is_none())
        {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    pub version: u8,
    pub permit: SignedDocument,
    pub payload_bytes: PayloadBytes,
}

/// Signature and binding proof only; parent/current policy/claim checks remain required.
pub struct VerifiedCommand {
    permit: GatewayOperationPermit,
    payload_bytes: PayloadBytes,
    payload: OperationPayload,
}
impl VerifiedCommand {
    pub fn permit(&self) -> &GatewayOperationPermit {
        &self.permit
    }
    pub fn payload_bytes(&self) -> &PayloadBytes {
        &self.payload_bytes
    }
    pub fn payload(&self) -> &OperationPayload {
        &self.payload
    }
}

pub struct MessageExpectation {
    issuer: String,
    audience: String,
    application: String,
    gateway_id: String,
    kid: String,
}
impl MessageExpectation {
    pub fn new(
        issuer: impl Into<String>,
        audience: impl Into<String>,
        application: impl Into<String>,
        gateway_id: impl Into<String>,
        kid: impl Into<String>,
    ) -> Result<Self, SecurityError> {
        let issuer = issuer.into();
        let audience = audience.into();
        let application = application.into();
        let gateway_id = gateway_id.into();
        let kid = kid.into();
        PrincipalRef::new(&issuer, PrincipalKind::Machine, "message-verifier")?;
        if kid.len() > 128
            || [&audience, &application, &gateway_id, &kid]
                .into_iter()
                .any(|s| !valid_id(s))
        {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self {
            issuer,
            audience,
            application,
            gateway_id,
            kid,
        })
    }
}

fn sign<T: Serialize>(
    claims: &T,
    key: &jsonwebtoken::EncodingKey,
    kid: &str,
    typ: &str,
) -> Result<SignedDocument, SecurityError> {
    if !valid_id(kid) || kid.len() > 128 {
        return Err(SecurityError::InvalidCredential);
    }
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.typ = Some(typ.into());
    header.kid = Some(kid.into());
    SignedDocument::new(
        jsonwebtoken::encode(&header, claims, key).map_err(|_| SecurityError::InvalidCredential)?,
    )
}
fn decode<T: DeserializeOwned>(
    signed: &SignedDocument,
    expected: &MessageExpectation,
    key: &jsonwebtoken::DecodingKey,
    typ: &str,
) -> Result<T, SecurityError> {
    let kid = super::credential::restricted_jws_key_id(signed.expose_secret(), typ)?;
    if kid != expected.kid {
        return Err(SecurityError::InvalidCredential);
    }
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(&[&expected.issuer]);
    validation.set_audience(&[&expected.audience]);
    validation.leeway = 0;
    validation.validate_exp = false;
    Ok(
        jsonwebtoken::decode::<T>(signed.expose_secret(), key, &validation)
            .map_err(|_| SecurityError::InvalidCredential)?
            .claims,
    )
}
fn validate_times(iat: u64, exp: u64, clock: &dyn Clock) -> Result<(), SecurityError> {
    let now = healthy_reading(clock)?
        .unix_millis
        .checked_add(CLOCK_ERROR_MILLIS)
        .ok_or(SecurityError::DependencyUnavailable)?;
    let issued = iat
        .checked_mul(1000)
        .ok_or(SecurityError::InvalidCredential)?;
    let expires = exp
        .checked_mul(1000)
        .ok_or(SecurityError::InvalidCredential)?;
    if exp <= iat || issued > now || expires <= now {
        return Err(SecurityError::InvalidCredential);
    }
    Ok(())
}

pub fn sign_gateway_operation(
    permit: &GatewayOperationPermit,
    key: &jsonwebtoken::EncodingKey,
    kid: &str,
) -> Result<SignedDocument, SecurityError> {
    permit.validate()?;
    sign(permit, key, kid, PERMIT_TYPE)
}
pub fn verify_command(
    envelope: &CommandEnvelope,
    expected: &MessageExpectation,
    key: &jsonwebtoken::DecodingKey,
    clock: &dyn Clock,
) -> Result<VerifiedCommand, SecurityError> {
    healthy_reading(clock)?;
    if envelope.version != 1 {
        return Err(SecurityError::InvalidCredential);
    }
    let permit: GatewayOperationPermit = decode(&envelope.permit, expected, key, PERMIT_TYPE)?;
    permit.validate()?;
    validate_times(permit.iat, permit.exp, clock)?;
    permit.authorization.ensure_fresh(clock)?;
    if permit.application != expected.application
        || resource_gateway(&permit.resource) != Some(&expected.gateway_id)
        || permit.request_digest != envelope.payload_bytes.digest()
    {
        return Err(SecurityError::InvalidCredential);
    }
    let payload: OperationPayload = serde_json::from_slice(envelope.payload_bytes.as_bytes())
        .map_err(|_| SecurityError::InvalidResource)?;
    payload.validate()?;
    if payload.request_id != permit.request_id
        || payload.task_id != permit.task_id
        || payload.operation != permit.operation
        || payload.resource != permit.resource
        || payload.capability_revision != permit.capability_revision
        || payload.target_binding_revision != permit.target_binding_revision
    {
        return Err(SecurityError::InvalidCredential);
    }
    Ok(VerifiedCommand {
        permit,
        payload_bytes: envelope.payload_bytes.clone(),
        payload,
    })
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOutcome {
    Succeeded,
    DeviceRejected,
    Unknown,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskResultMessage {
    pub device_id: String,
    pub task_id: Uuid,
    pub claim_id: Uuid,
    pub executor_instance_id: Uuid,
    pub request_digest: RequestDigest,
    pub outcome: ExecutionOutcome,
    pub result_payload: serde_json::Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceEventMessage {
    pub device_id: String,
    pub binding_revision: String,
    pub event_stream_id: Uuid,
    pub sequence: u64,
    pub state_payload: serde_json::Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GatewayEvent {
    TaskResult(TaskResultMessage),
    DeviceSnapshot(DeviceEventMessage),
    Heartbeat(DeviceEventMessage),
    Offline(DeviceEventMessage),
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayMessage {
    pub version: u8,
    pub iss: String,
    pub aud: String,
    pub application: String,
    pub gateway_id: String,
    pub kid: String,
    pub event_id: Uuid,
    pub iat: u64,
    pub exp: u64,
    pub event: GatewayEvent,
}
impl GatewayMessage {
    fn validate(&self) -> Result<(), SecurityError> {
        PrincipalRef::new(&self.iss, PrincipalKind::Machine, &self.gateway_id)?;
        if self.version != 1
            || self.event_id.is_nil()
            || self.exp <= self.iat
            || [&self.application, &self.gateway_id, &self.kid, &self.aud]
                .into_iter()
                .any(|s| !valid_id(s))
        {
            return Err(SecurityError::InvalidCredential);
        }
        match &self.event {
            GatewayEvent::TaskResult(result) => {
                if !valid_id(&result.device_id)
                    || result.task_id.is_nil()
                    || result.claim_id.is_nil()
                    || result.executor_instance_id.is_nil()
                {
                    return Err(SecurityError::InvalidCredential);
                }
            }
            GatewayEvent::DeviceSnapshot(event)
            | GatewayEvent::Heartbeat(event)
            | GatewayEvent::Offline(event) => {
                if !valid_id(&event.device_id)
                    || !valid_id(&event.binding_revision)
                    || event.event_stream_id.is_nil()
                {
                    return Err(SecurityError::InvalidCredential);
                }
            }
        }
        Ok(())
    }
}
pub struct VerifiedGatewayMessage(GatewayMessage);
impl VerifiedGatewayMessage {
    pub fn message(&self) -> &GatewayMessage {
        &self.0
    }
}
pub fn sign_gateway_message(
    message: &GatewayMessage,
    key: &jsonwebtoken::EncodingKey,
    kid: &str,
) -> Result<SignedDocument, SecurityError> {
    message.validate()?;
    if message.kid != kid {
        return Err(SecurityError::InvalidCredential);
    }
    sign(message, key, kid, EVENT_TYPE)
}
/// Cryptographic proof only. The receiver still checks registered key state, claim and replay sequence.
pub fn verify_gateway_message(
    signed: &SignedDocument,
    expected: &MessageExpectation,
    key: &jsonwebtoken::DecodingKey,
    clock: &dyn Clock,
) -> Result<VerifiedGatewayMessage, SecurityError> {
    healthy_reading(clock)?;
    let message: GatewayMessage = decode(signed, expected, key, EVENT_TYPE)?;
    message.validate()?;
    validate_times(message.iat, message.exp, clock)?;
    if message.gateway_id != expected.gateway_id
        || message.application != expected.application
        || message.kid != expected.kid
    {
        return Err(SecurityError::InvalidCredential);
    }
    Ok(VerifiedGatewayMessage(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        security::{budget::tests::ManualClock, identity::PrincipalKind, policy::ResourceTarget},
        token_manager::IdTokenSigner,
    };
    use rsa::{
        RsaPrivateKey,
        pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
        rand_core::OsRng,
    };
    use std::sync::OnceLock;
    fn keys() -> &'static (
        IdTokenSigner,
        jsonwebtoken::EncodingKey,
        jsonwebtoken::DecodingKey,
    ) {
        static KEYS: OnceLock<(
            IdTokenSigner,
            jsonwebtoken::EncodingKey,
            jsonwebtoken::DecodingKey,
        )> = OnceLock::new();
        KEYS.get_or_init(|| {
            let key = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
            let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
            let public = key
                .to_public_key()
                .to_public_key_pem(LineEnding::LF)
                .unwrap();
            (
                IdTokenSigner::from_rsa_pem(pem.as_bytes(), 3600).unwrap(),
                jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
                jsonwebtoken::DecodingKey::from_rsa_pem(public.as_bytes()).unwrap(),
            )
        })
    }
    fn fixture() -> (GatewayOperationPermit, PayloadBytes) {
        let payload = OperationPayload {
            version: 1,
            request_id: uuid::Uuid::new_v4(),
            task_id: Some(uuid::Uuid::new_v4()),
            operation: OperationId::new("task:execute").unwrap(),
            resource: ResourceRef::new(
                "app",
                ResourceTarget::Tool {
                    gateway_id: "g".into(),
                    device_id: "d".into(),
                    tool_name: "move".into(),
                },
            )
            .unwrap(),
            tool_name: Some("move".into()),
            capability_revision: Some("cap-1".into()),
            target_binding_revision: Some("target-1".into()),
            input: serde_json::json!({"speed":1}),
            headers: Default::default(),
            timeout_ms: Some(5000),
            http_method: None,
            route_id: None,
        };
        let bytes = PayloadBytes::from_payload(&payload).unwrap();
        let permit = GatewayOperationPermit {
            iss: "https://idp.test".into(),
            aud: "cobotlet:g".into(),
            jti: "permit-1".into(),
            iat: 100,
            exp: 105,
            token_use: TokenUse::GatewayOperation,
            actor: PrincipalRef::new("https://idp.test", PrincipalKind::User, "42").unwrap(),
            presenter_service: PrincipalRef::new(
                "https://idp.test",
                PrincipalKind::Machine,
                "cloud",
            )
            .unwrap(),
            parent_credential: CredentialRef::new("https://idp.test", "parent-1").unwrap(),
            grant_id: "grant-1".into(),
            application: "app".into(),
            request_id: payload.request_id,
            task_id: payload.task_id,
            operation: payload.operation,
            resource: payload.resource,
            request_digest: bytes.digest(),
            capability_revision: payload.capability_revision,
            target_binding_revision: payload.target_binding_revision,
            authorization: DecisionValidity::from_query_start(100_000, 200_000, None, None)
                .unwrap(),
        };
        (permit, bytes)
    }
    fn expected(audience: &str) -> MessageExpectation {
        MessageExpectation::new("https://idp.test", audience, "app", "g", "cobotrix-rsa-1").unwrap()
    }
    #[test]
    fn security_t09_command_signature_and_exact_payload_binding() {
        let (permit, payload) = fixture();
        let envelope = CommandEnvelope {
            version: 1,
            permit: keys().0.sign_gateway_operation(&permit).unwrap(),
            payload_bytes: payload.clone(),
        };
        let verified = verify_command(
            &envelope,
            &expected("cobotlet:g"),
            &keys().2,
            &ManualClock::new(),
        )
        .unwrap();
        assert_eq!(verified.payload_bytes().as_bytes(), payload.as_bytes());
        let raw = serde_json::to_vec(&envelope).unwrap();
        let roundtrip: CommandEnvelope = serde_json::from_slice(&raw).unwrap();
        assert!(
            verify_command(
                &roundtrip,
                &expected("cobotlet:g"),
                &keys().2,
                &ManualClock::new()
            )
            .is_ok()
        );
        for field in [
            "resource",
            "operation",
            "input",
            "headers",
            "timeout_ms",
            "capability_revision",
            "target_binding_revision",
            "task_id",
            "tool_name",
        ] {
            let mut raw: serde_json::Value = serde_json::from_slice(payload.as_bytes()).unwrap();
            raw[field] = serde_json::json!("tampered");
            let altered = CommandEnvelope {
                version: 1,
                permit: envelope.permit.clone(),
                payload_bytes: PayloadBytes::from_bytes(serde_json::to_vec(&raw).unwrap()).unwrap(),
            };
            assert!(
                verify_command(
                    &altered,
                    &expected("cobotlet:g"),
                    &keys().2,
                    &ManualClock::new()
                )
                .is_err(),
                "field {field}"
            );
        }
        assert!(!format!("{envelope:?}").contains(envelope.permit.expose_secret()));
    }
    #[test]
    fn security_t09_payload_and_permit_metadata_cannot_disagree_even_when_signed() {
        let (permit, payload) = fixture();
        for field in [
            "request_id",
            "task_id",
            "operation",
            "resource",
            "capability_revision",
            "target_binding_revision",
        ] {
            let mut raw = serde_json::to_value(&permit).unwrap();
            raw[field] = match field {
                "request_id" | "task_id" => serde_json::json!(uuid::Uuid::new_v4()),
                "operation" => serde_json::json!("device:read"),
                "resource" => {
                    serde_json::json!({"application":"app","target":{"kind":"tool","gateway_id":"g","device_id":"other","tool_name":"move"}})
                }
                _ => serde_json::json!("other"),
            };
            let altered: GatewayOperationPermit = serde_json::from_value(raw).unwrap();
            let signed = sign_gateway_operation(&altered, &keys().1, "cobotrix-rsa-1").unwrap();
            let command = CommandEnvelope {
                version: 1,
                permit: signed,
                payload_bytes: payload.clone(),
            };
            assert!(
                verify_command(
                    &command,
                    &expected("cobotlet:g"),
                    &keys().2,
                    &ManualClock::new()
                )
                .is_err(),
                "field {field}"
            );
        }
    }
    #[test]
    fn security_t09_expiry_purpose_and_gateway_identity_are_enforced() {
        let (permit, payload) = fixture();
        let envelope = CommandEnvelope {
            version: 1,
            permit: keys().0.sign_gateway_operation(&permit).unwrap(),
            payload_bytes: payload,
        };
        let clock = ManualClock::new();
        clock.advance(4000);
        assert!(verify_command(&envelope, &expected("cobotlet:g"), &keys().2, &clock).is_err());
        assert!(
            verify_command(
                &envelope,
                &expected("cobotlet:other"),
                &keys().2,
                &ManualClock::new()
            )
            .is_err()
        );
        assert!(
            IdTokenSigner::from_hmac("test", 3600)
                .sign_gateway_operation(&permit)
                .is_err()
        );
        assert!(
            verify_gateway_message(
                &envelope.permit,
                &expected("cobotlet:g"),
                &keys().2,
                &ManualClock::new()
            )
            .is_err()
        );
    }
    #[test]
    fn security_t09_signed_result_is_not_an_execution_permit() {
        let result = GatewayMessage {
            version: 1,
            iss: "https://idp.test".into(),
            aud: "cobotsrv".into(),
            application: "app".into(),
            gateway_id: "g".into(),
            kid: "cobotrix-rsa-1".into(),
            event_id: uuid::Uuid::new_v4(),
            iat: 100,
            exp: 160,
            event: GatewayEvent::TaskResult(TaskResultMessage {
                device_id: "d".into(),
                task_id: uuid::Uuid::new_v4(),
                claim_id: uuid::Uuid::new_v4(),
                executor_instance_id: uuid::Uuid::new_v4(),
                request_digest: RequestDigest::of(b"request"),
                outcome: ExecutionOutcome::Succeeded,
                result_payload: serde_json::json!({"ok":true}),
            }),
        };
        let signed = sign_gateway_message(&result, &keys().1, "cobotrix-rsa-1").unwrap();
        assert!(
            verify_gateway_message(
                &signed,
                &expected("cobotsrv"),
                &keys().2,
                &ManualClock::new()
            )
            .is_ok()
        );
        let (_, payload) = fixture();
        assert!(
            verify_command(
                &CommandEnvelope {
                    version: 1,
                    permit: signed,
                    payload_bytes: payload
                },
                &expected("cobotlet:g"),
                &keys().2,
                &ManualClock::new()
            )
            .is_err()
        );
        let wrong = MessageExpectation::new(
            "https://idp.test",
            "cobotsrv",
            "app",
            "other",
            "cobotrix-rsa-1",
        )
        .unwrap();
        let signed = sign_gateway_message(&result, &keys().1, "cobotrix-rsa-1").unwrap();
        assert!(verify_gateway_message(&signed, &wrong, &keys().2, &ManualClock::new()).is_err());
    }

    #[test]
    fn security_t09_nonsecond_query_start_and_wire_limits() {
        let (mut permit, payload) = fixture();
        permit.authorization =
            DecisionValidity::from_query_start(100_123, 200_000, None, None).unwrap();
        let clock = ManualClock::new();
        clock.advance(123);
        let command = CommandEnvelope {
            version: 1,
            permit: keys().0.sign_gateway_operation(&permit).unwrap(),
            payload_bytes: payload.clone(),
        };
        assert!(verify_command(&command, &expected("cobotlet:g"), &keys().2, &clock).is_ok());
        assert!(PayloadBytes::from_bytes(vec![0; MAX_PAYLOAD + 1]).is_err());
        assert!(RequestDigest::try_from("a".repeat(44)).is_err());
        let mut raw = payload.as_bytes().to_vec();
        raw.push(b' ');
        let command = CommandEnvelope {
            version: 1,
            permit: command.permit,
            payload_bytes: PayloadBytes::from_bytes(raw).unwrap(),
        };
        assert!(verify_command(&command, &expected("cobotlet:g"), &keys().2, &clock).is_err());
    }

    #[test]
    fn security_t09_unsupported_jose_extensions_are_not_ignored() {
        use crate::security::credential::restricted_jws_key_id;
        for extension in [
            serde_json::json!({"crit":["b64"],"b64":false}),
            serde_json::json!({"jku":"https://attacker.test"}),
        ] {
            let mut header = serde_json::json!({"alg":"RS256","typ":PERMIT_TYPE,"kid":"key-1"});
            header
                .as_object_mut()
                .unwrap()
                .extend(extension.as_object().unwrap().clone());
            let raw = format!(
                "{}.payload.signature",
                URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap())
            );
            assert!(restricted_jws_key_id(&raw, PERMIT_TYPE).is_err());
        }
    }
}
