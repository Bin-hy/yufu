use http::{HeaderMap, Uri, header};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyLoggingPolicy {
    Omit,
}

/// Body logging is opt-out of data collection, not a list of currently known endpoints.
/// New routes stay protected even when the API prefix or lifecycle endpoints change.
pub fn body_logging_policy(_path: &str) -> BodyLoggingPolicy {
    BodyLoggingPolicy::Omit
}
pub fn body_log_marker() -> &'static str {
    "[BODY OMITTED]"
}

/// Query *names* can also carry secrets (e.g. ?opaque-token without '=').
/// Log registered route templates where possible; raw URIs omit the complete query.
pub fn redact_uri(uri: &Uri) -> String {
    let path = uri.path();
    if uri.query().is_some() {
        format!("{path}?[REDACTED]")
    } else {
        path.to_owned()
    }
}

/// Keep only bounded, recognized transport metadata; never forward unknown headers
/// into logs, even if they are not named Authorization/Cookie.
pub fn redact_headers(headers: &HeaderMap) -> HeaderMap {
    let mut safe = HeaderMap::new();
    for name in [header::CONTENT_TYPE, header::ACCEPT] {
        if let Some(value) = headers.get(&name)
            && matches!(
                value.to_str(),
                Ok("application/json"
                    | "text/event-stream"
                    | "application/octet-stream"
                    | "application/x-www-form-urlencoded")
            )
        {
            safe.insert(name, value.clone());
        }
    }
    if let Some(length) = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        && let Ok(value) = length.to_string().parse()
    {
        safe.insert(header::CONTENT_LENGTH, value);
    }
    safe
}

/// Config field names are trusted schema metadata. All leaf values are opaque secrets
/// for logging purposes; do not maintain a fragile secret-key-name blacklist.
pub fn redact_config(value: &Value) -> Value {
    match value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) => Value::String("[REDACTED]".into()),
        Value::Array(items) => Value::Array(items.iter().map(redact_config).collect()),
        Value::Object(items) => Value::Object(
            items
                .iter()
                .map(|(key, value)| (key.clone(), redact_config(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Never log userinfo, paths, query strings or fragments from connection strings.
pub fn redact_service_url(value: &str) -> String {
    let Ok(url) = reqwest::Url::parse(value) else {
        return "[REDACTED]".into();
    };
    let Some(host) = url.host() else {
        return "[REDACTED]".into();
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!("{}://{host}{port}/[REDACTED]", url.scheme())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing::{
        Event, Metadata, Subscriber,
        span::{Attributes, Id, Record},
    };
    #[derive(Clone)]
    struct Capture(Arc<Mutex<String>>);
    impl tracing::field::Visit for Capture {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0
                .lock()
                .unwrap()
                .push_str(&format!("{}={value:?}", field.name()));
        }
    }
    impl Subscriber for Capture {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _: &Id, _: &Record<'_>) {}
        fn record_follows_from(&self, _: &Id, _: &Id) {}
        fn event(&self, event: &Event<'_>) {
            event.record(&mut self.clone());
        }
        fn enter(&self, _: &Id) {}
        fn exit(&self, _: &Id) {}
    }
    #[test]
    fn security_t10_capture_contains_no_secrets_from_uri_headers_config_or_body() {
        let capture = Capture(Arc::new(Mutex::new(String::new())));
        let mut headers = http::HeaderMap::new();
        for key in [
            "authorization",
            "cookie",
            "set-cookie",
            "x-api-key",
            "x-custom",
        ] {
            headers.insert(key, "secret-marker".parse().unwrap());
        }
        headers.insert("content-type", "application/json".parse().unwrap());
        let uri:http::Uri="https://user:secret-marker@example.test/private?access%5Ftoken=secret-marker&secret-marker&code=secret-marker".parse().unwrap();
        let config = serde_json::json!({"sync_client_secret":"secret-marker","nested":{"url":"postgres://user:secret-marker@localhost/db"},"enabled":true});
        tracing::subscriber::with_default(capture.clone(), || {
            tracing::info!(uri=%redact_uri(&uri),headers=?redact_headers(&headers),config=?redact_config(&config),body=%body_log_marker(),url=%redact_service_url("postgres://user:secret-marker@localhost/db?password=secret-marker"));
        });
        let output = capture.0.lock().unwrap();
        assert!(!output.is_empty());
        assert!(!output.contains("secret-marker"));
        assert!(output.contains("application/json"));
        assert!(output.contains("REDACTED"));
    }
    #[test]
    fn security_t10_sensitive_and_unknown_bodies_default_to_omitted() {
        for path in [
            "/oidc/token",
            "/custom/v2/oidc/mcp-tokens",
            "/custom/v2/oidc/mcp-tokens/3/refresh",
            "/custom/v2/application/sync-permissions",
            "/unknown",
        ] {
            assert_eq!(body_logging_policy(path), BodyLoggingPolicy::Omit);
        }
        assert_eq!(redact_service_url("not a URL: secret-marker"), "[REDACTED]");
        let headers = http::HeaderMap::from_iter([(
            http::header::CONTENT_TYPE,
            "secret-marker".parse().unwrap(),
        )]);
        assert!(!format!("{:?}", redact_headers(&headers)).contains("secret-marker"));
        let numeric =
            redact_config(&serde_json::json!({"password":123456789,"nested":[987654321,true]}));
        assert!(!numeric.to_string().contains("123456789"));
        assert!(!numeric.to_string().contains("987654321"));
    }
}
