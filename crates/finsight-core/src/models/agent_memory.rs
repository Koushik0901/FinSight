use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all="camelCase")]
pub struct AgentMemory {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub merchant_key: Option<String>,
    pub created_at: String,
}
