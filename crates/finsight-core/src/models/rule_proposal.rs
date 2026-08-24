use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all="camelCase")]
pub struct RuleProposal {
    pub id: String,
    pub when_label: String,
    pub description: String,
    pub pattern: String,
    pub category_id: String,
    pub status: String,
    pub created_at: String,
}
