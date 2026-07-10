use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{AccountOwner, HouseholdMember};
use finsight_core::repos::{household, run};

#[tauri::command]
#[specta::specta]
pub async fn list_household_members(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<HouseholdMember>> {
    let db = (*state.db).clone();
    run(&db, household::list_members)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_household_member(
    state: tauri::State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> AppResult<HouseholdMember> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        household::create_member(conn, &name, color.as_deref())
    })
    .await
    .map_err(AppError::from)
}

/// Mark one member as the operator ("self") of this install, then re-run the
/// classification cascade so existing data reflects the identity immediately:
/// the operator's OWN e-transfers become internal moves (out of income/expense
/// and off the anomaly list), which is what makes the savings rate correct.
/// Passing a non-existent id clears self.
#[tauri::command]
#[specta::specta]
pub async fn set_self_member(state: tauri::State<'_, AppState>, member_id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        household::set_self_member(conn, &member_id)?;
        finsight_core::categorize::apply_builtin_categorization(conn)?;
        finsight_core::categorize::pair_transfers(conn)?;
        finsight_core::anomaly::recompute_anomalies(conn)?;
        Ok::<_, finsight_core::CoreError>(())
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_household_member(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| household::delete_member(conn, &id))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_account_owners(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<AccountOwner>> {
    let db = (*state.db).clone();
    run(&db, household::list_account_owners)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_account_owners(
    state: tauri::State<'_, AppState>,
    account_id: String,
    member_ids: Vec<String>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        household::set_account_owners(conn, &account_id, &member_ids)
    })
    .await
    .map_err(AppError::from)
}
