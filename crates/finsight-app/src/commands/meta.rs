use crate::error::AppResult;

pub use finsight_api::commands::meta::AppReady;

#[tauri::command]
#[specta::specta]
pub async fn app_ready() -> AppResult<AppReady> {
    finsight_api::commands::meta::app_ready().await
}
