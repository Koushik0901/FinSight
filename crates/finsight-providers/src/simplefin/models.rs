use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleFinAccountSet {
    #[serde(default)]
    pub accounts: Vec<SimpleFinAccount>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub errlist: Vec<SimpleFinError>,
    #[serde(default)]
    pub connections: Vec<SimpleFinConnection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleFinError {
    pub code: String,
    pub msg: String,
    #[serde(rename = "conn_id")]
    pub connection_id: Option<String>,
    #[serde(rename = "account_id")]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleFinConnection {
    #[serde(rename = "conn_id")]
    pub conn_id: String,
    pub name: String,
    #[serde(rename = "org_id")]
    pub org_id: String,
    #[serde(rename = "org_url")]
    pub org_url: Option<String>,
    #[serde(rename = "sfin_url")]
    pub sfin_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleFinAccount {
    pub id: String,
    pub name: String,
    #[serde(alias = "org", alias = "conn_name")]
    pub connection_name: Option<String>,
    #[serde(rename = "conn_id")]
    pub connection_id: Option<String>,
    pub currency: String,
    pub balance: String,
    #[serde(rename = "available-balance")]
    pub available_balance: Option<String>,
    #[serde(rename = "balance-date")]
    pub balance_date: i64,
    #[serde(default)]
    pub transactions: Option<Vec<SimpleFinTransaction>>,
    #[serde(default)]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleFinTransaction {
    pub id: String,
    pub posted: i64,
    pub transacted_at: Option<i64>,
    pub amount: String,
    pub description: String,
    #[serde(default)]
    pub payee: String,
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub extra: Option<Value>,
}

impl SimpleFinTransaction {
    pub fn posted_at_epoch(&self) -> i64 {
        self.posted
    }

    pub fn transacted_at_epoch(&self) -> Option<i64> {
        self.transacted_at
    }
}
