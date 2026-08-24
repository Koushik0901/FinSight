use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
pub struct Rule {
    pub id: String,
    pub pattern: String,
    pub category_id: String,
    pub enabled: bool,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub treatment: String,
}

#[derive(Debug, Clone)]
pub struct NewRule {
    pub pattern: String,
    pub category_id: String,
    pub source: String,
    pub treatment: String,
}
