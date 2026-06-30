use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Security {
    pub id: String,
    pub connection_id: String,
    pub external_security_id: String,
    pub ticker_symbol: Option<String>,
    pub name: Option<String>,
    pub currency: Option<String>,
}
