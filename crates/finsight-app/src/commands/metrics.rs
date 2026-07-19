//! Frontend bridge to the shared `finsight-core::metrics` layer. Screens read
//! canonical balances, averages, runway, and targets from here rather than
//! recomputing them client-side, so the UI and the Copilot never disagree.

use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::metrics::{
    FinancialAssumptionsInput, FinancialMetrics, MemberNetWorth,
};

/// `get_financial_metrics`, optionally scoped to one household member. A `None`
/// member returns the whole-household numbers (unchanged); `Some(id)` weights
/// every figure by the member's ownership share (explicit `share_bps`, else an
/// equal split), so the per-person view reconciles to the household total plus
/// the unassigned residual.
#[tauri::command]
#[specta::specta]
pub async fn get_financial_metrics(
    state: tauri::State<'_, AppState>,
    member_id: Option<String>,
) -> AppResult<FinancialMetrics> {
    finsight_api::commands::metrics::get_financial_metrics(&state.api, member_id).await
}

/// Each household member's share of net worth (share-weighted across accounts AND
/// jointly-owned assets, via the metrics layer — NOT a client-side equal split),
/// plus an "unassigned" residual so the rows sum to the household total.
#[tauri::command]
#[specta::specta]
pub async fn household_net_worth_breakdown(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<MemberNetWorth>> {
    finsight_api::commands::metrics::household_net_worth_breakdown(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_financial_assumptions(
    state: tauri::State<'_, AppState>,
    input: FinancialAssumptionsInput,
) -> AppResult<()> {
    finsight_api::commands::metrics::set_financial_assumptions(&state.api, input).await
}
