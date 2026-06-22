use serde::Deserialize;

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
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleFinConnection {
    pub conn_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleFinAccount {
    pub id: String,
    pub name: String,
    #[serde(alias = "org", alias = "conn_name")]
    pub connection_name: Option<String>,
    #[serde(rename = "conn_id")]
    pub connection_id: Option<String>,
    pub currency: String,
    pub balance: String,
    #[serde(rename = "balance-date")]
    pub balance_date: i64,
    #[serde(default)]
    pub transactions: Option<Vec<SimpleFinTransaction>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleFinTransaction {
    pub id: String,
    pub posted: i64,
    pub transacted_at: Option<i64>,
    pub amount: String,
    pub description: String,
    #[serde(default)]
    pub payee: String,
}

impl SimpleFinTransaction {
    pub fn posted_at_epoch(&self) -> i64 {
        self.posted
    }

    pub fn transacted_at_epoch(&self) -> Option<i64> {
        self.transacted_at
    }
}
