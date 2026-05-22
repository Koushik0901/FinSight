use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Cleared,
    Pending,
    Manual,
}

impl TransactionStatus {
    pub fn as_db(&self) -> &'static str {
        match self {
            Self::Cleared => "cleared",
            Self::Pending => "pending",
            Self::Manual => "manual",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "manual" => Self::Manual,
            _ => Self::Cleared,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub merchant_raw: String,
    pub merchant_id: Option<String>,
    pub merchant_label: Option<String>,
    pub merchant_color: Option<String>,
    pub merchant_initials: Option<String>,
    pub category_id: Option<String>,
    pub category_label: Option<String>,
    pub category_color: Option<String>,
    pub status: TransactionStatus,
    pub notes: Option<String>,
    pub ai_confidence: Option<f64>,
    pub ai_explanation: Option<String>,
    pub is_anomaly: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct NewTransaction {
    pub account_id: String,
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub merchant_raw: String,
    pub category_id: Option<String>,
    pub notes: Option<String>,
    pub status: TransactionStatus,
}
