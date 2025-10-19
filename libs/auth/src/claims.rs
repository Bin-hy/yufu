use serde::{Deserialize, Serialize};
use std::fmt::Display;
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Claims {
    pub id: String,
    pub iat: u64, // token created time
    pub exp: u64, // expiration
}

impl Display for Claims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {} expire:{}", self.id, self.exp)
    }
}
