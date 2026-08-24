use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
pub struct Categorization {
    pub id: String,
    pub txn_id: String,
    pub category_id: Option<String>,
    pub source: String,
    pub confidence: f64,
    pub model: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewCategorization {
    pub txn_id: String,
    pub category_id: Option<String>,
    pub source: String,
    pub confidence: f64,
    pub model: Option<String>,
}
