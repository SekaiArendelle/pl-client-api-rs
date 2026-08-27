use serde::Serialize;
use serde_json::Value;

/// A community content category understood by the Physics-Lab API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    Experiment,
    Discussion,
}

/// The user fields included in the authentication response.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: String,
    pub nickname: Option<String>,
    pub signature: Option<String>,
    pub is_bound: bool,
    pub gold: i64,
    pub level: i64,
    pub avatar: i64,
    pub avatar_region: i64,
    pub decoration: i64,
    pub verification: Value,
}
