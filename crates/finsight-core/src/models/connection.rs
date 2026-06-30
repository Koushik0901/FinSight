use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SimpleFinConnection {
    pub id: String,
    pub access_url_ref: String,
    pub conn_id: Option<String>,
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub org_url: Option<String>,
    pub sfin_url: Option<String>,
    pub label: Option<String>,
    pub status: String,
    pub last_error: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct NewSimpleFinConnection {
    pub access_url_ref: String,
    pub conn_id: Option<String>,
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub org_url: Option<String>,
    pub sfin_url: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
pub struct SimpleFinConnectionPatch {
    pub status: Option<String>,
    pub last_error: Option<Option<String>>,
    pub last_synced_at: Option<Option<DateTime<Utc>>>,
    pub label: Option<Option<String>>,
    pub org_name: Option<Option<String>>,
}
