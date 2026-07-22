//! Codegen wrappers for the unified notification policy.

use crate::error::AppResult;
use crate::AppState;
use finsight_core::notify::Notification;

pub use finsight_api::commands::notifications::{
    NotificationCategoryPref, NotificationPrefsDto, QuietHours,
};

#[tauri::command]
#[specta::specta]
pub async fn get_notification_prefs(state: tauri::State<'_, AppState>) -> AppResult<NotificationPrefsDto> {
    finsight_api::commands::notifications::get_notification_prefs(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_notification_prefs(
    state: tauri::State<'_, AppState>,
    prefs: NotificationPrefsDto,
) -> AppResult<()> {
    finsight_api::commands::notifications::set_notification_prefs(&state.api, prefs).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_notifications(
    state: tauri::State<'_, AppState>,
    include_resolved: Option<bool>,
) -> AppResult<Vec<Notification>> {
    finsight_api::commands::notifications::list_notifications(&state.api, include_resolved).await
}

#[tauri::command]
#[specta::specta]
pub async fn mark_notification_read(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::notifications::mark_notification_read(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn mark_all_notifications_read(state: tauri::State<'_, AppState>) -> AppResult<u32> {
    finsight_api::commands::notifications::mark_all_notifications_read(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn notification_unread_count(state: tauri::State<'_, AppState>) -> AppResult<i64> {
    finsight_api::commands::notifications::notification_unread_count(&state.api).await
}
