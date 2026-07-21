use crate::{error::AppResult, AppState};

pub use finsight_api::commands::push::{PushDeliveryReport, PushDevice, PushStatus};

#[tauri::command]
#[specta::specta]
pub async fn get_push_status(state: tauri::State<'_, AppState>) -> AppResult<PushStatus> {
    finsight_api::commands::push::get_push_status(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_push_subscription(
    state: tauri::State<'_, AppState>,
    endpoint: String,
    p256dh: String,
    auth: String,
    label: Option<String>,
) -> AppResult<()> {
    finsight_api::commands::push::save_push_subscription(&state.api, endpoint, p256dh, auth, label)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_push_subscription(
    state: tauri::State<'_, AppState>,
    endpoint: String,
) -> AppResult<bool> {
    finsight_api::commands::push::delete_push_subscription(&state.api, endpoint).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_push_devices(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<PushDevice>> {
    finsight_api::commands::push::list_push_devices(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn send_test_push(
    state: tauri::State<'_, AppState>,
) -> AppResult<PushDeliveryReport> {
    finsight_api::commands::push::send_test_push(&state.api).await
}
