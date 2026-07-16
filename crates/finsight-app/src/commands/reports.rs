use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::reports::{
    CategoryTotal, CreateMonthlyReviewInput, MerchantTotal, MonthSummary, MonthTotals,
    MonthlyReview, MonthlyReviewSnapshot, ReportData, SavingsRatePoint,
};

#[tauri::command]
#[specta::specta]
pub async fn get_report_data(
    state: tauri::State<'_, AppState>,
    scope: String,
    member_id: Option<String>,
) -> AppResult<ReportData> {
    finsight_api::commands::reports::get_report_data(&state.api, scope, member_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_month_totals(state: tauri::State<'_, AppState>) -> AppResult<MonthTotals> {
    finsight_api::commands::reports::get_month_totals(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_savings_rate_history(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SavingsRatePoint>> {
    finsight_api::commands::reports::get_savings_rate_history(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_monthly_review(
    state: tauri::State<'_, AppState>,
    input: CreateMonthlyReviewInput,
) -> AppResult<MonthlyReview> {
    finsight_api::commands::reports::create_monthly_review(&state.api, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_monthly_reviews(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<MonthlyReview>> {
    finsight_api::commands::reports::list_monthly_reviews(&state.api).await
}
