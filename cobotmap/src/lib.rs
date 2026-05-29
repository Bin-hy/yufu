use std::path::Path;
use url::Url;

mod app;
mod config;
mod routes;
mod upstream;

pub use app::{run, verify_maps_js_url};
pub use config::Config;

pub const DEFAULT_SOURCE_URL: &str = "https://maps.googleapis.com/maps/api/js";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyConfig(Url);

impl ProxyConfig {
    pub fn from_url(value: &str) -> Result<Self, String> {
        if value.trim().is_empty() {
            return Err("proxy URL must not be empty".to_string());
        }

        let url = Url::parse(value.trim()).map_err(|err| format!("bad proxy URL: {err}"))?;
        match url.scheme() {
            "socks5" | "socks5h" => Ok(Self(url)),
            scheme => Err(format!(
                "unsupported proxy scheme: {scheme} (expected socks5:// or socks5h://)"
            )),
        }
    }

    pub fn url(&self) -> &Url {
        &self.0
    }
}

impl<'de> serde::Deserialize<'de> for ProxyConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_url(&value).map_err(serde::de::Error::custom)
    }
}

pub fn load_maps_from_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read source file {}: {err}", path.display()))
}

pub fn build_maps_source_url(api_key: &str) -> String {
    let mut url = Url::parse(DEFAULT_SOURCE_URL).expect("DEFAULT_SOURCE_URL must be valid");
    url.query_pairs_mut()
        .append_pair("key", api_key)
        .append_pair("callback", "initMap");
    url.into()
}

pub(crate) fn redact_url(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return value.to_string();
    };

    let query: Vec<_> = url
        .query_pairs()
        .map(|(key, value)| {
            let value = match key.as_ref() {
                "key" | "token" | "signature" => "[redacted]".into(),
                _ => value,
            };
            (key.into_owned(), value.into_owned())
        })
        .collect();

    if !query.is_empty() {
        url.query_pairs_mut().clear().extend_pairs(query);
    }

    url.into()
}
