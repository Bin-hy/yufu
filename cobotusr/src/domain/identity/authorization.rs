//! Strict protocol input parsing; constructing a request grants no redirect trust.
use super::{application::IdentityScopeSet, error::IdentityError, session::Prompt};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct AuthorizationParameters(BTreeMap<String, String>);
impl AuthorizationParameters {
    pub fn parse(query: &str) -> Result<Self, IdentityError> {
        let fields = strict_form(query, 16_384)?;
        let parameters = Self(fields);
        parameters.required("client_id")?;
        parameters.required("redirect_uri")?;
        Ok(parameters)
    }
    pub fn from_payload(payload: serde_json::Value) -> Result<Self, IdentityError> {
        let fields: BTreeMap<String, String> =
            serde_json::from_value(payload).map_err(|_| IdentityError::DependencyUnavailable)?;
        if fields.len() > 16
            || fields
                .iter()
                .any(|(k, v)| k.len() > 128 || v.len() > 2048 || v.chars().any(char::is_control))
        {
            return Err(IdentityError::DependencyUnavailable);
        }
        Ok(Self(fields))
    }
    pub fn client_id(&self) -> Result<&str, IdentityError> {
        self.required("client_id")
    }
    pub fn redirect_uri(&self) -> Result<&str, IdentityError> {
        self.required("redirect_uri")
    }
    pub fn state(&self) -> &str {
        self.0.get("state").map(String::as_str).unwrap_or("")
    }
    pub fn optional_state(&self) -> Option<String> {
        self.0.get("state").cloned()
    }
    pub fn validation_error_code(&self) -> &'static str {
        if self
            .0
            .get("response_type")
            .is_some_and(|value| value != "code")
        {
            "unsupported_response_type"
        } else if self
            .0
            .get("scope")
            .is_some_and(|value| IdentityScopeSet::parse(value).is_err())
        {
            "invalid_scope"
        } else {
            "invalid_request"
        }
    }
    pub fn to_payload(&self) -> serde_json::Value {
        serde_json::json!(self.0)
    }
    pub fn validate(&self) -> Result<AuthorizationRequest, IdentityError> {
        const SUPPORTED: &[&str] = &[
            "client_id",
            "redirect_uri",
            "response_type",
            "response_mode",
            "scope",
            "state",
            "nonce",
            "code_challenge",
            "code_challenge_method",
            "prompt",
            "max_age",
        ];
        if self.0.keys().any(|key| !SUPPORTED.contains(&key.as_str()))
            || self.required("response_type")? != "code"
            || self
                .0
                .get("response_mode")
                .is_some_and(|mode| mode != "query")
            || self.required("code_challenge_method")? != "S256"
        {
            return Err(IdentityError::InvalidInput);
        }
        let challenge = self.required("code_challenge")?;
        let bytes = URL_SAFE_NO_PAD
            .decode(challenge)
            .map_err(|_| IdentityError::InvalidInput)?;
        if bytes.len() != 32 || URL_SAFE_NO_PAD.encode(bytes) != challenge {
            return Err(IdentityError::InvalidInput);
        }
        let max_age = self
            .0
            .get("max_age")
            .map(|value| {
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(IdentityError::InvalidInput);
                }
                value
                    .parse::<u64>()
                    .map_err(|_| IdentityError::InvalidInput)
            })
            .transpose()?;
        Ok(AuthorizationRequest {
            client_id: self.required("client_id")?.into(),
            redirect_uri: self.required("redirect_uri")?.into(),
            scopes: IdentityScopeSet::parse(self.required("scope")?)?,
            state: self.required("state")?.into(),
            nonce: self.required("nonce")?.into(),
            code_challenge: challenge.into(),
            prompt: Prompt::parse(self.0.get("prompt").map(String::as_str))?,
            max_age,
        })
    }
    fn required(&self, field: &str) -> Result<&str, IdentityError> {
        self.0
            .get(field)
            .map(String::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 2048 && !s.chars().any(char::is_control))
            .ok_or(IdentityError::InvalidInput)
    }
}

#[derive(Clone)]
pub struct AuthorizationRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: IdentityScopeSet,
    pub state: String,
    pub nonce: String,
    pub code_challenge: String,
    pub prompt: Prompt,
    pub max_age: Option<u64>,
}

/// Unlike lossy URL parsers, reject malformed percent escapes and invalid UTF-8.
/// Also reject duplicate parameters, even when their spellings differ before decoding.
pub fn strict_form(raw: &str, max_bytes: usize) -> Result<BTreeMap<String, String>, IdentityError> {
    strict_form_with_value_limit(raw, max_bytes, 2048)
}

pub(crate) fn strict_form_with_value_limit(
    raw: &str,
    max_bytes: usize,
    max_value: usize,
) -> Result<BTreeMap<String, String>, IdentityError> {
    if raw.is_empty() || raw.len() > max_bytes {
        return Err(IdentityError::InvalidInput);
    }
    let mut fields = BTreeMap::new();
    for pair in raw.split('&') {
        let (key, value) = pair.split_once('=').ok_or(IdentityError::InvalidInput)?;
        let key = decode_component(key)?;
        let value = decode_component(value)?;
        if key.is_empty()
            || key.len() > 128
            || value.len() > max_value
            || fields.insert(key, value).is_some()
            || fields.len() > 16
        {
            return Err(IdentityError::InvalidInput);
        }
    }
    Ok(fields)
}
fn decode_component(raw: &str) -> Result<String, IdentityError> {
    let source = raw.as_bytes();
    let mut decoded = Vec::with_capacity(source.len());
    let mut i = 0;
    while i < source.len() {
        match source[i] {
            b'%' => {
                let pair = source
                    .get(i + 1..i + 3)
                    .ok_or(IdentityError::InvalidInput)?;
                let high = (pair[0] as char)
                    .to_digit(16)
                    .ok_or(IdentityError::InvalidInput)?;
                let low = (pair[1] as char)
                    .to_digit(16)
                    .ok_or(IdentityError::InvalidInput)?;
                decoded.push((high * 16 + low) as u8);
                i += 3;
            }
            b'+' => {
                decoded.push(b' ');
                i += 1;
            }
            byte => {
                decoded.push(byte);
                i += 1;
            }
        }
    }
    let value = String::from_utf8(decoded).map_err(|_| IdentityError::InvalidInput)?;
    if value.chars().any(char::is_control) {
        return Err(IdentityError::InvalidInput);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parameters() -> AuthorizationParameters {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("client_id", "web-dev"),
                ("redirect_uri", "https://rp.example/cb?original=1"),
                ("response_type", "code"),
                ("scope", "openid profile"),
                ("state", "s + &"),
                ("nonce", "request-nonce"),
                (
                    "code_challenge",
                    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                ),
                ("code_challenge_method", "S256"),
            ])
            .finish();
        AuthorizationParameters::parse(&query).unwrap()
    }
    #[test]
    fn identity_protocol_request_preserves_exact_fields_and_revalidates_payload() {
        let parameters = parameters();
        let request = parameters.validate().unwrap();
        assert_eq!(request.state, "s + &");
        assert_eq!(request.redirect_uri, "https://rp.example/cb?original=1");
        assert_eq!(request.scopes.as_protocol_string(), "openid profile");
        assert_eq!(
            AuthorizationParameters::from_payload(parameters.to_payload())
                .unwrap()
                .validate()
                .unwrap()
                .nonce,
            "request-nonce"
        );
    }
    #[test]
    fn identity_protocol_request_rejects_unsupported_and_missing_security_parameters() {
        for (field, value) in [
            ("response_type", "token"),
            ("response_mode", "fragment"),
            ("scope", "openid gateway"),
            ("code_challenge_method", "plain"),
            (
                "code_challenge",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB",
            ),
            ("prompt", "none login"),
            ("prompt", "consent consent"),
            ("max_age", "-1"),
            ("max_age", "18446744073709551616"),
            ("resource", "https://api.example"),
            ("request_uri", "https://request.example"),
            ("state", ""),
            ("nonce", ""),
        ] {
            let mut parameters = parameters();
            parameters.0.insert(field.into(), value.into());
            assert!(parameters.validate().is_err(), "accepted {field}");
        }
        for field in [
            "state",
            "nonce",
            "code_challenge",
            "code_challenge_method",
            "scope",
            "response_type",
        ] {
            let mut parameters = parameters();
            parameters.0.remove(field);
            assert!(parameters.validate().is_err(), "missing {field}");
        }
        for value in ["none", "login", "consent", "login consent"] {
            let mut parameters = parameters();
            parameters.0.insert("prompt".into(), value.into());
            assert!(parameters.validate().is_ok());
        }
    }
    #[test]
    fn identity_protocol_request_rejects_duplicate_malformed_and_control_fields() {
        for query in [
            "client_id=one&%63lient_id=two",
            "a=%",
            "a=%0",
            "a=%XZ",
            "a=%FF",
            "a=%00",
            "a=ok&&b=yes",
            "a",
            "=value",
        ] {
            assert!(
                strict_form(query, 16_384).is_err(),
                "accepted malformed input"
            );
        }
        assert!(strict_form(&format!("a={}", "a".repeat(2049)), 16_384).is_err());
        assert_eq!(
            strict_form("a=%E4%BD%A0%E5%A5%BD+world", 16_384).unwrap()["a"],
            "你好 world"
        );
    }
}
