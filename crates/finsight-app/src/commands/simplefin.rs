use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::keychain::{self, SIMPLEFIN_SERVICE, SIMPLEFIN_USER};
use finsight_core::models::{AccountType, NewAccount};
use finsight_core::repos::accounts;
use finsight_core::repos::run;
use finsight_providers::simplefin::{fetch_simplefin_data, commit_simplefin_import, SimpleFinClient, SimpleFinAccount};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinStatus {
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinAccountInfo {
    pub id: String,
    pub name: String,
    pub connection_name: String,
    pub currency: String,
    pub balance: String,
}

impl From<SimpleFinAccount> for SimpleFinAccountInfo {
    fn from(a: SimpleFinAccount) -> Self {
        Self {
            id: a.id,
            name: a.name,
            connection_name: a.connection_name.unwrap_or_else(|| "Unknown".to_string()),
            currency: a.currency,
            balance: a.balance,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinAccountImportRequest {
    pub simplefin_id: String,
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub added: usize,
    pub skipped: usize,
}

#[tauri::command]
#[specta::specta]
pub async fn save_simplefin_setup_token(
    _state: tauri::State<'_, AppState>,
    token: String,
) -> AppResult<()> {
    let access_url = SimpleFinClient::claim_token(&token)
        .await
        .map_err(AppError::from)?;
    keychain::set_key(SIMPLEFIN_SERVICE, SIMPLEFIN_USER, &access_url).map_err(AppError::from)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_simplefin_status(
    _state: tauri::State<'_, AppState>,
) -> AppResult<SimpleFinStatus> {
    let configured = keychain::get_key(SIMPLEFIN_SERVICE, SIMPLEFIN_USER)
        .map_err(AppError::from)?
        .is_some();
    Ok(SimpleFinStatus { configured })
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_accounts(
    _state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SimpleFinAccountInfo>> {
    let access_url = keychain::get_key(SIMPLEFIN_SERVICE, SIMPLEFIN_USER)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::new("simplefin.not_configured", "SimpleFin not configured"))?;
    let client = SimpleFinClient::new(&access_url).map_err(AppError::from)?;
    let (accounts, connections) = client.list_accounts_with_connections().await.map_err(AppError::from)?;
    Ok(accounts
        .into_iter()
        .map(|a| {
            let connection_name = a.connection_name.clone().or_else(|| {
                a.connection_id.as_ref().and_then(|conn_id| {
                    connections
                        .iter()
                        .find(|c| &c.conn_id == conn_id)
                        .map(|c| c.name.clone())
                })
            });
            SimpleFinAccountInfo {
                id: a.id,
                name: a.name,
                connection_name: connection_name.unwrap_or_else(|| "Unknown".to_string()),
                currency: a.currency,
                balance: a.balance,
            }
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn import_simplefin_accounts(
    state: tauri::State<'_, AppState>,
    accounts: Vec<SimpleFinAccountImportRequest>,
) -> AppResult<Vec<String>> {
    let access_url = keychain::get_key(SIMPLEFIN_SERVICE, SIMPLEFIN_USER)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::new("simplefin.not_configured", "SimpleFin not configured"))?;
    let db = state.db.clone();

    let mut created_ids: Vec<String> = Vec::new();
    for req in &accounts {
        let local_id = run(&db, {
            let nickname = req.nickname.clone();
            let simplefin_id = req.simplefin_id.clone();
            move |conn| {
                let name = nickname.clone().unwrap_or_else(|| simplefin_id.clone());
                let account = NewAccount {
                    owner: "Me".to_string(),
                    bank: "SimpleFin".to_string(),
                    r#type: AccountType::Checking,
                    name,
                    last4: None,
                    currency: "USD".to_string(),
                    color: "#C9F950".to_string(),
                    opening_balance_cents: 0,
                    source: "simplefin".to_string(),
                    liquidity_type: "liquid".to_string(),
                    emergency_fund_eligible: true,
                    goal_earmark: None,
                    apy_pct: None,
                    simplefin_account_id: Some(simplefin_id.clone()),
                    nickname,
                };
                accounts::insert(conn, account)
            }
        })
        .await
        .map_err(AppError::from)?;
        created_ids.push(local_id.id);
    }

    for (req, local_id) in accounts.iter().zip(created_ids.iter()) {
        let local_id = local_id.clone();
        match fetch_simplefin_data(&access_url, &req.simplefin_id, &local_id).await {
            Ok(pending) => {
                let lid = local_id.clone();
                let summary = run(&db, {
                    move |conn| {
                        commit_simplefin_import(pending, conn)
                            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
                    }
                })
                .await
                .map_err(AppError::from)?;
                tracing::info!(
                    "Initial sync for {}: added {}, skipped {}",
                    lid,
                    summary.added,
                    summary.skipped
                );
            }
            Err(e) => {
                tracing::error!("Failed to sync account {local_id}: {e}");
            }
        }
    }

    Ok(created_ids)
}

#[tauri::command]
#[specta::specta]
pub async fn sync_simplefin_account(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<SyncSummary> {
    let access_url = keychain::get_key(SIMPLEFIN_SERVICE, SIMPLEFIN_USER)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::new("simplefin.not_configured", "SimpleFin not configured"))?;
    let db = state.db.clone();

    let simplefin_id = run(&db, {
        let account_id = account_id.clone();
        move |conn| {
            let id: String = conn
                .query_row(
                    "SELECT simplefin_account_id FROM accounts WHERE id = ?1 AND archived_at IS NULL",
                    [&account_id],
                    |r| r.get(0),
                )
                .map_err(|e| finsight_core::CoreError::Database(e))?;
            Ok::<_, finsight_core::CoreError>(id)
        }
    })
    .await
    .map_err(AppError::from)?;

    let pending = fetch_simplefin_data(&access_url, &simplefin_id, &account_id)
        .await
        .map_err(AppError::from)?;

    let summary = run(&db, move |conn| {
        commit_simplefin_import(pending, conn)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)?;

    Ok(SyncSummary {
        added: summary.added,
        skipped: summary.skipped,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn disconnect_simplefin(state: tauri::State<'_, AppState>) -> AppResult<()> {
    keychain::delete_key(SIMPLEFIN_SERVICE, SIMPLEFIN_USER).map_err(AppError::from)?;
    let db = state.db.clone();
    run(&db, |conn| {
        conn.execute(
            "UPDATE accounts SET simplefin_account_id = NULL WHERE simplefin_account_id IS NOT NULL",
            [],
        )?;
        Ok::<_, finsight_core::CoreError>(())
    })
    .await
    .map_err(AppError::from)?;
    Ok(())
}
