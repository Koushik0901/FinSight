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
    pub simplefin_account_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub nickname: Option<String>,
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
    pub source: String,
    pub simplefin_account_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub nickname: Option<String>,
}

fn default_source() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
pub struct AccountPatch {
    pub name: Option<String>,
    pub bank: Option<String>,
    pub account_type: Option<AccountType>,
    pub color: Option<String>,
    pub last4: Option<Option<String>>,
    pub currency: Option<String>,
    pub nickname: Option<Option<String>>,
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
    #[serde(default = "default_source")]
    pub source: String,
    pub simplefin_account_id: Option<String>,
    pub nickname: Option<String>,
}
