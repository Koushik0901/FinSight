use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Holding {
    pub id: String,
    pub account_id: String,
    pub security_id: String,
    pub quantity: Option<f64>,
    pub cost_basis_cents: Option<i64>,
    pub market_value_cents: Option<i64>,
    pub currency: Option<String>,
    pub as_of_date: String,
}
