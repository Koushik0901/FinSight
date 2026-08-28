use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

/// One month's hold — amount parked for next month.
///
/// Money parked in `month` reduces that month's `to_budget` (`income - budgeted - hold`)
/// and is added to `available_funds` for the following month as income-like.
/// This mirrors Actual's `Hold` primitive: an auditable, reversible store of
/// "not yet assigned" rather than silently inflating a category.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct BudgetHold {
    /// "YYYY-MM"
    pub month: String,
    pub amount_cents: i64,
}

/// Request body for `POST /api/rpc/set_hold`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetHoldRequest {
    pub month: String,
    pub amount_cents: i64,
}

/// Request body for `POST /api/rpc/get_hold`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct GetHoldRequest {
    pub month: String,
}
