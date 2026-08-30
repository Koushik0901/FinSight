use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SyncRun {
    pub id: String,
    pub trigger: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub accounts_total: i64,
    pub accounts_succeeded: i64,
    pub accounts_failed: i64,
    pub added: i64,
    pub updated: i64,
    pub skipped: i64,
    pub queued_for_review: i64,
    pub error_summary: Option<String>,
}
