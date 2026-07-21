use crate::AppState;
use finsight_api::error::AppResult;
pub use finsight_api::commands::restoration::RestorationEnvelopeInput;
use finsight_core::repos::restoration::{RestorationEnvelope, RestorationLeg, RestorationStatus};

#[tauri::command]
#[specta::specta]
pub async fn list_restoration_envelopes(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<RestorationEnvelope>> {
    finsight_api::commands::restoration::list_restoration_envelopes(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_restoration_status(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<Option<RestorationStatus>> {
    finsight_api::commands::restoration::get_restoration_status(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_restoration_envelope(
    state: tauri::State<'_, AppState>,
    input: RestorationEnvelopeInput,
) -> AppResult<RestorationEnvelope> {
    finsight_api::commands::restoration::create_restoration_envelope(&state.api, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn close_restoration_envelope(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    finsight_api::commands::restoration::close_restoration_envelope(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_restoration_envelope(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    finsight_api::commands::restoration::delete_restoration_envelope(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_restoration_leg(
    state: tauri::State<'_, AppState>,
    envelope_id: String,
    amount_cents: i64,
    noted_on: String,
    transaction_id: Option<String>,
) -> AppResult<RestorationLeg> {
    finsight_api::commands::restoration::add_restoration_leg(
        &state.api,
        envelope_id,
        amount_cents,
        noted_on,
        transaction_id,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_restoration_leg(
    state: tauri::State<'_, AppState>,
    leg_id: String,
) -> AppResult<()> {
    finsight_api::commands::restoration::remove_restoration_leg(&state.api, leg_id).await
}
