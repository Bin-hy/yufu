pub mod config;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tracing::info;

// ── Wire types (what we POST to cobotlet) ─────────────────────────────────────

#[derive(Debug, Serialize)]
struct RegisterRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    service_url: &'a str,
    capabilities: Vec<CapabilityPayload<'a>>,
}

#[derive(Debug, Serialize)]
struct CapabilityPayload<'a> {
    name: &'a str,
    method: &'a str,
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    tags: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    input_schema: Option<&'a serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    id: String,
}

// ── Public entry point ────────────────────────────────────────────────────────

pub async fn run(cfg: config::Config) -> Result<()> {
    let cobotlet_url = cfg.cobotlet.url.trim_end_matches('/');

    let capabilities: Vec<CapabilityPayload<'_>> = cfg
        .device
        .capabilities
        .iter()
        .map(|c| CapabilityPayload {
            name: &c.name,
            method: &c.method,
            path: &c.path,
            description: c.description.as_deref(),
            tags: &c.tags,
            input_schema: c.input_schema.as_ref(),
        })
        .collect();

    let payload = RegisterRequest {
        id: cfg.device.id.as_deref(),
        service_url: &cfg.device.service_url,
        capabilities,
    };

    let url = format!("{cobotlet_url}/devices");
    info!(url, "registering device");

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .with_context(|| {
            format!(
                "cobotlet not reachable at {cobotlet_url} — is it running?\n  \
                 Check the [cobotlet] url in your config file"
            )
        })?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        bail!("cobotlet returned {status}: {body}");
    }

    let reg: RegisterResponse = resp.json().await.context("failed to parse response")?;

    let cap_names: Vec<&str> = payload.capabilities.iter().map(|c| c.name).collect();
    if cap_names.is_empty() {
        println!("registered device: {}", reg.id);
    } else {
        println!(
            "registered device: {}  capabilities: [{}]",
            reg.id,
            cap_names.join(", ")
        );
    }

    Ok(())
}

pub const EXAMPLE_CONFIG: &str = include_str!("../../conf/cobotset.example.toml");
