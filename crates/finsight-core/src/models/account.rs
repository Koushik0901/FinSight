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
    Loan,
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
            Self::Loan => "Loan",
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
            "Loan" => Self::Loan,
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
    pub liquidity_type: String,
    pub emergency_fund_eligible: bool,
    pub goal_earmark: Option<String>,
    pub apy_pct: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub simplefin_account_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub nickname: Option<String>,
    pub connection_id: Option<String>,
    pub institution_id: Option<String>,
    pub external_account_id: Option<String>,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    pub subtype: Option<String>,
    pub account_group: String,
    pub available_balance_cents: Option<i64>,
    pub balance_date: Option<DateTime<Utc>>,
    pub extra_json: Option<String>,
    pub raw_json: Option<String>,
    pub import_pending: bool,
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
    #[serde(default = "default_liquidity_type")]
    pub liquidity_type: String,
    #[serde(default = "default_emergency_fund_eligible")]
    pub emergency_fund_eligible: bool,
    pub goal_earmark: Option<String>,
    pub apy_pct: Option<f64>,
    pub simplefin_account_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub nickname: Option<String>,
    pub connection_id: Option<String>,
    pub institution_id: Option<String>,
    pub external_account_id: Option<String>,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    pub subtype: Option<String>,
    pub account_group: String,
    pub available_balance_cents: Option<i64>,
    pub balance_date: Option<DateTime<Utc>>,
    pub extra_json: Option<String>,
    pub raw_json: Option<String>,
    pub import_pending: bool,
}

fn default_source() -> String {
    "manual".to_string()
}

fn default_liquidity_type() -> String {
    "liquid".to_string()
}

fn default_emergency_fund_eligible() -> bool {
    true
}

fn default_account_group() -> String {
    "other".to_string()
}

fn default_import_pending() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalancePoint {
    pub date: String,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountSparkline {
    pub account_id: String,
    pub points: Vec<AccountBalancePoint>,
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
pub struct AccountPatch {
    pub name: Option<String>,
    pub bank: Option<String>,
    pub account_type: Option<AccountType>,
    pub color: Option<String>,
    pub last4: Option<Option<String>>,
    pub currency: Option<String>,
    pub liquidity_type: Option<String>,
    pub emergency_fund_eligible: Option<bool>,
    pub goal_earmark: Option<Option<String>>,
    pub apy_pct: Option<Option<f64>>,
    pub nickname: Option<Option<String>>,
    pub official_name: Option<Option<String>>,
    pub subtype: Option<Option<String>>,
    pub account_group: Option<String>,
    pub import_pending: Option<bool>,
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
    #[serde(default = "default_liquidity_type")]
    pub liquidity_type: String,
    #[serde(default = "default_emergency_fund_eligible")]
    pub emergency_fund_eligible: bool,
    pub goal_earmark: Option<String>,
    pub apy_pct: Option<f64>,
    pub simplefin_account_id: Option<String>,
    pub nickname: Option<String>,
    pub connection_id: Option<String>,
    pub institution_id: Option<String>,
    pub external_account_id: Option<String>,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    pub subtype: Option<String>,
    #[serde(default = "default_account_group")]
    pub account_group: String,
    pub available_balance_cents: Option<i64>,
    pub balance_date: Option<DateTime<Utc>>,
    pub extra_json: Option<String>,
    pub raw_json: Option<String>,
    #[serde(default = "default_import_pending")]
    pub import_pending: bool,
}
