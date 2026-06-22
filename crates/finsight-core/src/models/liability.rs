use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Liability {
    pub id: String,
    pub name: String,
    pub liability_type: String,
    pub balance_cents: i64,
    pub limit_cents: Option<i64>,
    pub apr_pct: Option<f64>,
    pub min_payment_cents: Option<i64>,
    pub payoff_date: Option<String>,
    pub original_balance_cents: Option<i64>,
    pub started_at: Option<String>,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewLiability {
    pub name: String,
    pub liability_type: String,
    pub balance_cents: i64,
    pub limit_cents: Option<i64>,
    pub apr_pct: Option<f64>,
    pub min_payment_cents: Option<i64>,
    pub payoff_date: Option<String>,
    pub original_balance_cents: Option<i64>,
    pub started_at: Option<String>,
    pub currency: String,
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityPatch {
    pub name: Option<String>,
    pub liability_type: Option<String>,
    pub balance_cents: Option<i64>,
    pub limit_cents: Option<Option<i64>>,
    pub apr_pct: Option<Option<f64>>,
    pub min_payment_cents: Option<Option<i64>>,
    pub payoff_date: Option<Option<String>>,
    pub original_balance_cents: Option<Option<i64>>,
    pub started_at: Option<Option<String>>,
    pub currency: Option<String>,
}
