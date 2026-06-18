use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTransaction {
    pub id: String,
    pub description: String,
    pub amount_cents: i64,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub due_date: String,
    pub status: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPlannedTransaction {
    pub description: String,
    pub amount_cents: i64,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub due_date: String,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct PlannedTxnFilter {
    pub status: Option<String>,
    pub due_before: Option<String>,
}
