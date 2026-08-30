use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SimpleFinAlert {
    pub id: String,
    pub account_id: String,
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub details_json: Option<String>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
