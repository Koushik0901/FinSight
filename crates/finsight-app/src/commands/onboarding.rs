use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::run;
use finsight_core::{sample, settings};
use serde::Serialize;
use specta::Type;

const KEY_COMPLETION: &str = "onboarding_completion_marked";

#[derive(Debug, Clone, Serialize, Type)]
pub struct OnboardingState {
    pub account_count: i64,
    pub category_count: i64,
    pub completion_marked: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn get_onboarding_state(state: tauri::State<'_, AppState>) -> AppResult<OnboardingState> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let account_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM accounts WHERE archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        let category_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM categories WHERE archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        let completion_marked: bool =
            settings::get::<bool>(conn, KEY_COMPLETION)?.unwrap_or(false);
        Ok(OnboardingState {
            account_count,
            category_count,
            completion_marked,
        })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn seed_sample_household(
    state: tauri::State<'_, AppState>,
) -> AppResult<sample::SeedSummary> {
    let db = (*state.db).clone();
    // seed_household finishes its own import row atomically inside its transaction.
    sample::seed_household(&db).map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn mark_onboarding_complete(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, |conn| settings::set(conn, KEY_COMPLETION, &true))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn reset_onboarding_completion(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, |conn| settings::set(conn, KEY_COMPLETION, &false))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn clear_sample_data(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        conn.execute("DELETE FROM accounts WHERE source = 'sample'", [])?;
        settings::set(conn, KEY_COMPLETION, &false)?;
        Ok(())
    })
    .await
    .map_err(AppError::from)
}
