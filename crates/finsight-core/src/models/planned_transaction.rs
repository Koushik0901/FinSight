use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewPlannedTransaction {
    pub description: String,
    pub amount_cents: i64,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub due_date: String,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedTransactionPatch {
    pub description: Option<String>,
    pub amount_cents: Option<i64>,
    pub account_id: Option<Option<String>>,
    pub category_id: Option<Option<String>>,
    pub due_date: Option<String>,
    pub status: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedTxnFilter {
    pub status: Option<String>,
    pub due_before: Option<String>,
}
