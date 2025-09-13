use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub token: String,
    pub kind: NodeKind,
    pub url: String,
}

impl Node {
    pub fn new(token: String, kind: NodeKind, url: String) -> Self {
        Self {
            token,
            kind,
            url,
        }
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
}

impl Storage {
    pub fn new(_client: reqwest::Client) -> Self {
        Self {
            list: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_map_nodes_mut(&self) -> Arc<RwLock<HashMap<String, Node>>> {
        self.list.clone()
    }

    pub fn get_map_nodes(&self) -> HashMap<String, Node> {
        self.list.read().unwrap().clone()
    }
}
