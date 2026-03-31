use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub token: String,
    pub kind: NodeKind,
    pub url: String,
}

impl Node {
    pub fn new(token: String, kind: NodeKind, url: String) -> Self {
        Self { token, kind, url }
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    #[default]
    #[serde(rename = "static")]
    Static,
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "net4mqtt")]
    Net4mqtt,
}

#[derive(Clone)]
pub struct Storage {
    list: Arc<RwLock<HashMap<String, Node>>>,
    codes: Arc<RwLock<HashMap<String, AuthCodeEntry>>>,
}

impl Storage {
    pub fn new(_client: reqwest::Client) -> Self {
        Self {
            list: Arc::new(RwLock::new(HashMap::new())),
            codes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_map_nodes_mut(&self) -> Arc<RwLock<HashMap<String, Node>>> {
        self.list.clone()
    }

    pub fn get_map_nodes(&self) -> HashMap<String, Node> {
        self.list.read().unwrap().clone()
    }

    pub fn insert_auth_code(&self, code: String, entry: AuthCodeEntry) {
        self.codes.write().unwrap().insert(code, entry);
    }

    pub fn take_auth_code(&self, code: &str) -> Option<AuthCodeEntry> {
        self.codes.write().unwrap().remove(code)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthCodeEntry {
    pub user_id: i32,
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String, // "plain" | "S256"
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
}
