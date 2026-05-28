use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Rule {
    pub id: String,
    pub pattern: String,
    pub category_id: String,
    pub enabled: bool,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewRule {
    pub pattern: String,
    pub category_id: String,
    pub source: String,
}
