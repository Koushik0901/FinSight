use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum AccountType {
    Checking,
    Savings,
    Credit,
    Investment,
    Cash,
    Other,
}

impl AccountType {
    pub fn as_db(&self) -> &'static str {
        match self {
            Self::Checking => "Checking",
            Self::Savings => "Savings",
            Self::Credit => "Credit",
            Self::Investment => "Investment",
            Self::Cash => "Cash",
            Self::Other => "Other",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "Checking" => Self::Checking,
            "Savings" => Self::Savings,
            "Credit" => Self::Credit,
            "Investment" => Self::Investment,
            "Cash" => Self::Cash,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Account {
    pub id: String,
    pub owner: String,
    pub bank: String,
    pub r#type: AccountType,
    pub name: String,
    pub last4: Option<String>,
    pub currency: String,
    pub color: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AccountSummary {
    pub id: String,
    pub owner: String,
    pub bank: String,
    pub r#type: AccountType,
    pub name: String,
    pub balance_cents: i64,
    pub currency: String,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct NewAccount {
    pub owner: String,
    pub bank: String,
    pub r#type: AccountType,
    pub name: String,
    pub last4: Option<String>,
    pub currency: String,
    pub color: String,
    pub opening_balance_cents: i64,
}
