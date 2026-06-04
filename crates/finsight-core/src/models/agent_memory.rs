use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemory {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub merchant_key: Option<String>,
    pub created_at: String,
}
