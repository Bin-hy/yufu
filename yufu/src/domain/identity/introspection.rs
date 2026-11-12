use serde::Serialize;

/// Inactive responses carry no identifying metadata.
#[derive(Debug, Serialize)]
pub struct Introspection {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
}
impl Introspection {
    pub fn inactive() -> Self {
        Self {
            active: false,
            scope: None,
            client_id: None,
            token_type: None,
            exp: None,
            sub: None,
            sid: None,
            aud: None,
            iss: None,
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn identity_introspection_inactive_has_no_metadata() {
        assert_eq!(
            serde_json::to_value(super::Introspection::inactive()).unwrap(),
            serde_json::json!({"active":false})
        );
    }
}
