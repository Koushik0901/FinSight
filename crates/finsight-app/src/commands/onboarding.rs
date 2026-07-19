use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::onboarding::{
    LlmProviderConfig, OllamaProbeResult, OnboardingState, StarterCategory,
};

#[tauri::command]
#[specta::specta]
pub async fn get_onboarding_state(state: tauri::State<'_, AppState>) -> AppResult<OnboardingState> {
    finsight_api::commands::onboarding::get_onboarding_state(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn mark_onboarding_complete(state: tauri::State<'_, AppState>) -> AppResult<()> {
    finsight_api::commands::onboarding::mark_onboarding_complete(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn reset_onboarding_completion(state: tauri::State<'_, AppState>) -> AppResult<()> {
    finsight_api::commands::onboarding::reset_onboarding_completion(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn probe_ollama(base_url: String) -> AppResult<OllamaProbeResult> {
    finsight_api::commands::onboarding::probe_ollama(base_url).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_llm_provider(
    state: tauri::State<'_, AppState>,
    config: LlmProviderConfig,
) -> AppResult<()> {
    finsight_api::commands::onboarding::save_llm_provider(&state.api, config).await
}

#[tauri::command]
#[specta::specta]
pub async fn commit_starter_categories(
    state: tauri::State<'_, AppState>,
    categories: Vec<StarterCategory>,
) -> AppResult<()> {
    finsight_api::commands::onboarding::commit_starter_categories(&state.api, categories).await
}
