use crate::{error::AppResult, AppState};

pub use finsight_api::commands::journey::{JourneyMilestone, JourneyStatus};

#[tauri::command]
#[specta::specta]
pub async fn get_journey_status(state: tauri::State<'_, AppState>) -> AppResult<JourneyStatus> {
    finsight_api::commands::journey::get_journey_status(&state.api).await
}
