use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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
