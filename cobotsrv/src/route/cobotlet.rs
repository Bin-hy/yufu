use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::AppState;
use crate::result::Result;
use auth::require_perm;

pub fn route() -> Router<AppState> {
    Router::new()
        .route("/cobotlets", get(list).layer(require_perm!("device:read")))
}

#[derive(Debug, Serialize)]
pub struct CobotletVO {
    pub alias: String,
    pub url: String,
    /// True if a non-empty MQTT online message has been received and no
    /// subsequent offline/LWT has cleared it.
    pub online: bool,
    pub last_seen: Option<String>,
}

async fn list(State(state): State<AppState>) -> Result<Json<Vec<CobotletVO>>> {
    // Build the set of MQTT-observed statuses (if MQTT is configured).
    let mqtt_status: std::collections::HashMap<String, crate::mqtt::CobotletStatus> =
        state
            .mqtt
            .as_ref()
            .map(|m| {
                m.cobotlet_status()
                    .into_iter()
                    .map(|s| (s.alias.clone(), s))
                    .collect()
            })
            .unwrap_or_default();

    let cobotlets: Vec<CobotletVO> = state
        .config
        .nodes
        .iter()
        .map(|node| {
            let status = mqtt_status.get(&node.alias);
            CobotletVO {
                alias: node.alias.clone(),
                url: node.url.clone(),
                online: status.map(|s| s.online).unwrap_or(false),
                last_seen: status.and_then(|s| {
                    s.last_seen.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                }),
            }
        })
        .collect();

    Ok(Json(cobotlets))
}
