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

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct StarterCategory {
    pub id: String,
    pub label: String,
    pub group_id: String,
}

#[tauri::command]
#[specta::specta]
pub async fn commit_starter_categories(
    state: tauri::State<'_, AppState>,
    categories: Vec<StarterCategory>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let tx = conn.transaction()?;
        for (gid, label) in [
            ("fixed", "Fixed"),
            ("daily", "Daily"),
            ("lifestyle", "Lifestyle"),
            ("wellbeing", "Wellbeing"),
        ] {
            tx.execute(
                "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES(?1, ?2, 0)",
                rusqlite::params![gid, label],
            )?;
        }
        for c in &categories {
            tx.execute(
                "INSERT OR IGNORE INTO categories(id, group_id, label, color, sort_order) \
                 VALUES(?1, ?2, ?3, '#94A3B8', 0)",
                rusqlite::params![c.id, c.group_id, c.label],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
    .map_err(AppError::from)
}
