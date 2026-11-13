use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessSourceKind {
    Direct,
    Group,
    Maintainer,
    TestUser,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccessSource {
    pub kind: AccessSourceKind,
    pub source_id: String,
    pub label: String,
}
