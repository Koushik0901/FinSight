use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::{AgentRecipe, AgentRecipeRun};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_recipes(
    state: State<'_, AppState>,
    include_paused: bool,
) -> AppResult<Vec<AgentRecipe>> {
    finsight_api::commands::recipes::list_recipes(&state.api, include_paused).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_recipe(
    state: State<'_, AppState>,
    title: String,
    description: String,
    recipe_kind: String,
    prompt_template: String,
    cadence: String,
    day_of_week: Option<i64>,
    day_of_month: Option<i64>,
) -> AppResult<AgentRecipe> {
    finsight_api::commands::recipes::create_recipe(
        &state.api,
        title,
        description,
        recipe_kind,
        prompt_template,
        cadence,
        day_of_week,
        day_of_month,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn update_recipe(
    state: State<'_, AppState>,
    id: String,
    title: String,
    description: String,
    prompt_template: String,
    cadence: String,
    day_of_week: Option<i64>,
    day_of_month: Option<i64>,
) -> AppResult<AgentRecipe> {
    finsight_api::commands::recipes::update_recipe(
        &state.api,
        id,
        title,
        description,
        prompt_template,
        cadence,
        day_of_week,
        day_of_month,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn pause_recipe(state: State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::recipes::pause_recipe(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn resume_recipe(state: State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::recipes::resume_recipe(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_recipe(state: State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::recipes::delete_recipe(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn trigger_recipe(state: State<'_, AppState>, id: String) -> AppResult<String> {
    finsight_api::commands::recipes::trigger_recipe(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_recipe_runs(
    state: State<'_, AppState>,
    recipe_id: String,
    limit: Option<u32>,
) -> AppResult<Vec<AgentRecipeRun>> {
    finsight_api::commands::recipes::list_recipe_runs(&state.api, recipe_id, limit).await
}
