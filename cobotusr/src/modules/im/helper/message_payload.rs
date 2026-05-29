use serde_json::{Map, Value};

pub use crate::modules::im::model::protocol::normalize_message_payload;

pub fn normalize_optional_content(content: Option<String>) -> Option<String> {
    content.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn merge_client_msg_id(extra: Option<Value>, client_msg_id: Option<String>) -> Option<Value> {
    let Some(client_msg_id) = client_msg_id else {
        return extra;
    };

    match extra {
        Some(Value::Object(mut map)) => {
            map.insert("client_msg_id".to_string(), Value::String(client_msg_id));
            Some(Value::Object(map))
        }
        Some(other) => {
            let mut map = Map::new();
            map.insert("client_msg_id".to_string(), Value::String(client_msg_id));
            map.insert("payload".to_string(), other);
            Some(Value::Object(map))
        }
        None => {
            let mut map = Map::new();
            map.insert("client_msg_id".to_string(), Value::String(client_msg_id));
            Some(Value::Object(map))
        }
    }
}
