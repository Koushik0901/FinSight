use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentRecipe {
    pub id: String,
    pub title: String,
    pub description: String,
    pub recipe_kind: String,
    pub prompt_template: String,
    pub cadence: String,
    pub day_of_week: Option<i64>,
    pub day_of_month: Option<i64>,
    pub status: String,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub run_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentRecipeRun {
    pub id: String,
    pub recipe_id: String,
    pub bundle_id: Option<String>,
    pub triggered_at: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
}
