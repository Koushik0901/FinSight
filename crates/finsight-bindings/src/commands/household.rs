use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::{AccountOwner, AssetOwner, HouseholdMember, OwnerShare};

#[tauri::command]
#[specta::specta]
pub async fn list_household_members(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<HouseholdMember>> {
    finsight_api::commands::household::list_household_members(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_household_member(
    state: tauri::State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> AppResult<HouseholdMember> {
    finsight_api::commands::household::create_household_member(&state.api, name, color).await
}

/// Mark one member as the operator ("self") of this install, then re-run the
/// classification cascade so existing data reflects the identity immediately:
/// the operator's OWN e-transfers become internal moves (out of income/expense
/// and off the anomaly list), which is what makes the savings rate correct.
/// Passing a non-existent id clears self.
#[tauri::command]
#[specta::specta]
pub async fn set_self_member(state: tauri::State<'_, AppState>, member_id: String) -> AppResult<()> {
    finsight_api::commands::household::set_self_member(&state.api, member_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_household_member(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    finsight_api::commands::household::delete_household_member(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_account_owners(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<AccountOwner>> {
    finsight_api::commands::household::list_account_owners(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_account_owners(
    state: tauri::State<'_, AppState>,
    account_id: String,
    member_ids: Vec<String>,
) -> AppResult<()> {
    finsight_api::commands::household::set_account_owners(&state.api, account_id, member_ids).await
}

/// Replace an account's owners with explicit per-owner shares (basis points;
/// null ⇒ equal split). Recomputing metrics is not needed — the weight is read
/// live from `share_bps` on every query.
#[tauri::command]
#[specta::specta]
pub async fn set_account_owner_shares(
    state: tauri::State<'_, AppState>,
    account_id: String,
    owners: Vec<OwnerShare>,
) -> AppResult<()> {
    finsight_api::commands::household::set_account_owner_shares(&state.api, account_id, owners)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_asset_owners(state: tauri::State<'_, AppState>) -> AppResult<Vec<AssetOwner>> {
    finsight_api::commands::household::list_asset_owners(&state.api).await
}

/// Replace a manual asset's owners with explicit per-owner shares (basis points;
/// null ⇒ equal split), so a jointly-owned house/car folds each owner's share
/// into their net worth.
#[tauri::command]
#[specta::specta]
pub async fn set_asset_owners(
    state: tauri::State<'_, AppState>,
    asset_id: String,
    owners: Vec<OwnerShare>,
) -> AppResult<()> {
    finsight_api::commands::household::set_asset_owners(&state.api, asset_id, owners).await
}
