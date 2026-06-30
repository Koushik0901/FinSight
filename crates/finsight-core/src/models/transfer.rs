use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransactionTransfer {
    pub id: String,
    pub from_transaction_id: String,
    pub to_transaction_id: String,
    pub confidence: String,
    pub detected_at: DateTime<Utc>,
    pub user_confirmed: bool,
}
