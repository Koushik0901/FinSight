use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleProposal {
    pub id: String,
    pub when_label: String,
    pub description: String,
    pub pattern: String,
    pub category_id: String,
    pub status: String,
    pub created_at: String,
}
