use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

/// One auditable budget transfer — Actual's Cover as a ledger row.
///
/// Moves `amount_cents` from `from_category` to `to_category` within `month`.
/// Either `from_category` or `to_category` may be `None` to represent moving
/// to/from unassigned (To Budget), but not both. At least one side is set and
/// `from != to` is enforced by the SQL CHECK. Net per-category effect is
/// `+transfers_in - transfers_out` and is included in
/// `available = budgeted + carryover + transfers_in - transfers_out - spent`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all="camelCase")]
pub struct BudgetTransfer {
    pub id: String,
    pub month: String,
    pub from_category: Option<String>,
    pub to_category: Option<String>,
    pub amount_cents: i64,
    pub note: Option<String>,
    pub created_at: String,
}

/// Request body for `POST /api/rpc/transfer_budget`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all="camelCase")]
pub struct TransferBudgetRequest {
    /// Category to move from. `None` means unassigned / To Budget.
    pub from_category: Option<String>,
    /// Category to move to. `None` means unassigned / To Budget.
    pub to_category: Option<String>,
    /// Amount in cents, must be > 0. Will be validated against available spare
    /// of `from_category` when `from_category` is Some.
    pub amount_cents: i64,
    /// "YYYY-MM"
    pub month: String,
    pub note: Option<String>,
}

/// Request body for `POST /api/rpc/list_budget_transfers`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all="camelCase")]
pub struct ListBudgetTransfersRequest {
    /// "YYYY-MM"
    pub month: String,
}
