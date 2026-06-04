use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualAsset {
    pub id: String,
    pub name: String,
    pub asset_type: String,
    pub value_cents: i64,
    pub currency: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewManualAsset {
    pub name: String,
    pub asset_type: String,
    pub value_cents: i64,
    pub currency: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualAssetPatch {
    pub name: Option<String>,
    pub asset_type: Option<String>,
    pub value_cents: Option<i64>,
    pub currency: Option<String>,
    pub notes: Option<Option<String>>,
}
