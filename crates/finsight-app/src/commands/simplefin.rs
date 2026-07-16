use crate::error::{AppError, AppResult};
use crate::AppState;
use chrono::{DateTime, Utc};
use finsight_core::models::{
    AccountType, ImportCandidateWithMatches, Institution as InstitutionModel, NewAccount,
    NewInstitution, NewSimpleFinConnection, SimpleFinAlert, SimpleFinConnection as DbConnection,
    SimpleFinConnectionPatch,
};
use finsight_core::repos::{
    accounts, alerts, connections, import_candidates, institutions, run, transfers,
};
use finsight_core::{keychain, settings};
use finsight_providers::simplefin::models::SimpleFinConnection as ProviderConnection;
use finsight_providers::simplefin::{
    classify_account, commit_simplefin_import, fetch_simplefin_data, SimpleFinClient,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

const SIMPLEFIN_ACCESS_SERVICE: &str = "com.finsight.simplefin.access";
const ONBOARDING_COMPLETION_KEY: &str = "onboarding_completion_marked";

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinStatus {
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinConnectionInfo {
    pub id: String,
    pub org_name: Option<String>,
    pub label: Option<String>,
    pub status: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinPurgeSummary {
    pub accounts_deleted: i64,
    pub transactions_deleted: i64,
    pub connections_deleted: i64,
}

impl From<DbConnection> for SimpleFinConnectionInfo {
    fn from(c: DbConnection) -> Self {
        Self {
            id: c.id,
            org_name: c.org_name,
            label: c.label,
            status: c.status,
            last_synced_at: c.last_synced_at,
            created_at: c.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinAccountInfo {
    pub id: String,
    pub name: String,
    pub connection_name: String,
    pub connection_id: String,
    pub currency: String,
    pub balance: String,
    pub account_type: AccountType,
    pub account_group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SimpleFinAccountImportRequest {
    pub simplefin_id: String,
    pub connection_id: String,
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub added: usize,
    pub updated: usize,
    pub skipped: usize,
    pub queued_for_review: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferSuggestionInfo {
    pub id: String,
    pub confidence: String,
    pub detected_at: DateTime<Utc>,
    pub from_transaction_id: String,
    pub from_account_name: String,
    pub from_merchant: String,
    pub from_amount_cents: i64,
    pub from_posted_at: DateTime<Utc>,
    pub to_transaction_id: String,
    pub to_account_name: String,
    pub to_merchant: String,
    pub to_amount_cents: i64,
    pub to_posted_at: DateTime<Utc>,
}

impl From<finsight_core::repos::transfers::TransferSuggestion> for TransferSuggestionInfo {
    fn from(s: finsight_core::repos::transfers::TransferSuggestion) -> Self {
        Self {
            id: s.id,
            confidence: s.confidence,
            detected_at: s.detected_at,
            from_transaction_id: s.from_transaction_id,
            from_account_name: s.from_account_name,
            from_merchant: s.from_merchant,
            from_amount_cents: s.from_amount_cents,
            from_posted_at: s.from_posted_at,
            to_transaction_id: s.to_transaction_id,
            to_account_name: s.to_account_name,
            to_merchant: s.to_merchant,
            to_amount_cents: s.to_amount_cents,
            to_posted_at: s.to_posted_at,
        }
    }
}

/// Claim a SimpleFin setup token and persist the resulting bridge access URL
/// plus every connection exposed by that access URL.
#[tauri::command]
#[specta::specta]
pub async fn save_simplefin_setup_token(
    state: tauri::State<'_, AppState>,
    token: String,
) -> AppResult<Vec<SimpleFinConnectionInfo>> {
    let access_url = SimpleFinClient::claim_token(&token)
        .await
        .map_err(AppError::from)?;

    // Validate and discover connections before persisting anything.
    let client = SimpleFinClient::new(&access_url).map_err(AppError::from)?;
    let (provider_accounts, provider_conns) = client
        .list_accounts_with_connections()
        .await
        .map_err(AppError::from)?;

    let bridge_id = Uuid::new_v4().to_string();
    keychain::set_key(SIMPLEFIN_ACCESS_SERVICE, &bridge_id, &access_url).map_err(AppError::from)?;

    let db = state.api.db.clone();
    let infos = run(&db, {
        let bridge_id = bridge_id.clone();
        let provider_conns = provider_conns.clone();
        move |conn| {
            let mut out: Vec<SimpleFinConnectionInfo> =
                Vec::with_capacity(std::cmp::max(1, provider_conns.len()));

            if provider_conns.is_empty() {
                // Preserve the access URL even if the bridge has no connections yet.
                let c = connections::insert(
                    conn,
                    NewSimpleFinConnection {
                        access_url_ref: bridge_id,
                        conn_id: None,
                        org_id: None,
                        org_name: Some("SimpleFin Bridge".to_string()),
                        org_url: None,
                        sfin_url: None,
                        label: None,
                    },
                )?;
                out.push(c.into());
            } else {
                for pc in provider_conns {
                    upsert_institution_from_connection(conn, &pc)?;
                    let c = connections::upsert_by_conn_id(
                        conn,
                        NewSimpleFinConnection {
                            access_url_ref: bridge_id.clone(),
                            conn_id: Some(pc.conn_id.clone()),
                            org_id: Some(pc.org_id.clone()),
                            org_name: Some(pc.name.clone()),
                            org_url: pc.org_url.clone(),
                            sfin_url: Some(pc.sfin_url.clone()),
                            label: Some(pc.name.clone()),
                        },
                    )?;
                    out.push(c.into());
                }
            }

            for account in provider_accounts {
                let Some(existing) = accounts::get_by_simplefin_id(conn, &account.id)? else {
                    continue;
                };
                let local_connection_id = account
                    .connection_id
                    .as_deref()
                    .and_then(|provider_conn_id| {
                        connections::find_by_conn_id(conn, provider_conn_id)
                            .ok()
                            .flatten()
                            .map(|c| c.id)
                    })
                    .or_else(|| out.first().map(|c| c.id.clone()));

                let Some(local_connection_id) = local_connection_id else {
                    continue;
                };
                let connection = connections::get(conn, &local_connection_id)?;
                let connection_name = account
                    .connection_name
                    .clone()
                    .or_else(|| {
                        connection
                            .label
                            .clone()
                            .or_else(|| connection.org_name.clone())
                    })
                    .unwrap_or_else(|| "SimpleFin".to_string());
                let (account_type, account_group) =
                    classify_account(&account, Some(&connection_name));

                let refreshed = NewAccount {
                    owner: existing.owner,
                    bank: connection_name,
                    r#type: account_type,
                    name: account.name.clone(),
                    last4: existing.last4,
                    currency: account.currency.clone(),
                    color: existing.color,
                    opening_balance_cents: 0,
                    source: "simplefin".to_string(),
                    liquidity_type: existing.liquidity_type,
                    emergency_fund_eligible: existing.emergency_fund_eligible,
                    goal_earmark: existing.goal_earmark,
                    apy_pct: existing.apy_pct,
                    simplefin_account_id: Some(account.id.clone()),
                    nickname: None,
                    connection_id: Some(local_connection_id),
                    institution_id: connection.org_id.clone(),
                    external_account_id: Some(account.id.clone()),
                    official_name: Some(account.name.clone()),
                    mask: existing.mask,
                    subtype: existing.subtype,
                    account_group: account_group.to_string(),
                    available_balance_cents: None,
                    balance_date: None,
                    extra_json: account.extra.as_ref().map(|v| v.to_string()),
                    raw_json: serde_json::to_string(&account).ok(),
                    import_pending: existing.import_pending,
                    // Debt fields are user-managed; preserve whatever the
                    // user already entered rather than wiping them on refresh.
                    apr_pct: existing.apr_pct,
                    min_payment_cents: existing.min_payment_cents,
                    payoff_date: existing.payoff_date,
                    limit_cents: existing.limit_cents,
                    original_balance_cents: existing.original_balance_cents,
                    started_at: existing.started_at,
                };
                let _ = accounts::upsert_simplefin_account(conn, refreshed)?;
            }

            Ok::<_, finsight_core::CoreError>(out)
        }
    })
    .await
    .map_err(AppError::from)?;

    Ok(infos)
}

#[tauri::command]
#[specta::specta]
pub async fn get_simplefin_status(state: tauri::State<'_, AppState>) -> AppResult<SimpleFinStatus> {
    let db = state.api.db.clone();
    let active = run(&db, |conn| {
        let conns = connections::list(conn)?;
        Ok::<_, finsight_core::CoreError>(conns.into_iter().any(|c| c.status == "active"))
    })
    .await
    .map_err(AppError::from)?;
    Ok(SimpleFinStatus { configured: active })
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_connections(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SimpleFinConnectionInfo>> {
    let db = state.api.db.clone();
    let infos = run(&db, |conn| {
        let conns = connections::list(conn)?;
        Ok::<_, finsight_core::CoreError>(
            conns
                .into_iter()
                .map(SimpleFinConnectionInfo::from)
                .collect(),
        )
    })
    .await
    .map_err(AppError::from)?;
    Ok(infos)
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_accounts(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SimpleFinAccountInfo>> {
    let db = state.api.db.clone();
    let connection_rows = run(&db, |conn| {
        Ok::<_, finsight_core::CoreError>(connections::list(conn)?)
    })
    .await
    .map_err(AppError::from)?;

    // Group local connection rows by the bridge access URL they share.
    let mut by_bridge: std::collections::HashMap<String, Vec<DbConnection>> =
        std::collections::HashMap::new();
    for c in connection_rows {
        by_bridge
            .entry(c.access_url_ref.clone())
            .or_default()
            .push(c);
    }

    let mut out = Vec::new();
    let mut failed_connections: Vec<(String, String)> = Vec::new();

    for (bridge_id, conns) in by_bridge {
        let access_url = match keychain::get_key(SIMPLEFIN_ACCESS_SERVICE, &bridge_id)
            .map_err(AppError::from)?
        {
            Some(url) => url,
            None => {
                let ids: Vec<String> = conns.iter().map(|c| c.id.clone()).collect();
                for id in ids {
                    failed_connections.push((id, "missing access url in keychain".to_string()));
                }
                continue;
            }
        };

        let client = match SimpleFinClient::new(&access_url) {
            Ok(c) => c,
            Err(e) => {
                let ids: Vec<String> = conns.iter().map(|c| c.id.clone()).collect();
                for id in ids {
                    failed_connections.push((id, e.to_string()));
                }
                continue;
            }
        };

        let (accounts, _) = match client.list_accounts_with_connections().await {
            Ok(a) => a,
            Err(e) => {
                let ids: Vec<String> = conns.iter().map(|c| c.id.clone()).collect();
                for id in ids {
                    failed_connections.push((id, e.to_string()));
                }
                continue;
            }
        };

        for sfin_account in accounts {
            let local_conn_id = conns
                .iter()
                .find(|c| c.conn_id.as_deref() == sfin_account.connection_id.as_deref())
                .map(|c| c.id.clone())
                .unwrap_or_else(|| conns.first().map(|c| c.id.clone()).unwrap_or_default());

            let connection_name = sfin_account
                .connection_name
                .clone()
                .or_else(|| {
                    conns
                        .iter()
                        .find(|c| c.conn_id.as_deref() == sfin_account.connection_id.as_deref())
                        .and_then(|c| c.label.clone().or_else(|| c.org_name.clone()))
                })
                .unwrap_or_else(|| "SimpleFin".to_string());

            let (account_type, account_group) =
                classify_account(&sfin_account, Some(&connection_name));

            out.push(SimpleFinAccountInfo {
                id: sfin_account.id,
                name: sfin_account.name,
                connection_name,
                connection_id: local_conn_id,
                currency: sfin_account.currency,
                balance: sfin_account.balance,
                account_type,
                account_group: account_group.to_string(),
            });
        }
    }

    // Mark failed connections so the UI can surface them.
    if !failed_connections.is_empty() {
        run(&db, {
            let failed = failed_connections.clone();
            move |conn| {
                for (id, err) in failed {
                    let _ = connections::update(
                        conn,
                        &id,
                        SimpleFinConnectionPatch {
                            status: Some("error".to_string()),
                            last_error: Some(Some(err)),
                            ..Default::default()
                        },
                    );
                }
                Ok::<_, finsight_core::CoreError>(())
            }
        })
        .await
        .map_err(AppError::from)?;
    }

    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub async fn import_simplefin_accounts(
    state: tauri::State<'_, AppState>,
    accounts: Vec<SimpleFinAccountImportRequest>,
) -> AppResult<Vec<String>> {
    let db = state.api.db.clone();

    // Snapshot the ledger epoch before the network fetch: importing accounts
    // inserts top-level account rows (no FK guard) that would survive a wipe.
    let start_epoch = db.reset_barrier().epoch();

    // Pull all active accounts from every connection so we can match selected ids.
    let remote_accounts = list_simplefin_accounts(state.clone()).await?;

    // Hold a reset lease across the account-creation loop and skip if a
    // Delete-All landed while we fetched. Released before the initial-sync loop
    // below (each `sync_local_account` takes its own lease — holding one across
    // that call would nest read leases and can deadlock a pending reset).
    let create_lease = db.reset_barrier().writer_lease(start_epoch).await;
    if create_lease.superseded() {
        return Err(AppError::new(
            "reset",
            "Import cancelled: all data was cleared during the import.",
        ));
    }

    let mut created_ids: Vec<String> = Vec::new();
    for req in &accounts {
        let remote = remote_accounts
            .iter()
            .find(|a| a.id == req.simplefin_id && a.connection_id == req.connection_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "simplefin.account_not_found",
                    format!("SimpleFin account {} not found", req.simplefin_id),
                )
            })?;

        let connection = run(&db, {
            let connection_id = req.connection_id.clone();
            move |conn| connections::get(conn, &connection_id)
        })
        .await
        .map_err(AppError::from)?;

        let local_id = run(&db, {
            let nickname = normalize_nickname(req.nickname.as_deref());
            let simplefin_id = req.simplefin_id.clone();
            let connection_id = req.connection_id.clone();
            let remote = remote.clone();
            move |conn| {
                let name = remote.name.clone();
                let bank = remote.connection_name.clone();
                let account_type = remote.account_type;
                let account = NewAccount {
                    owner: "Me".to_string(),
                    bank,
                    r#type: account_type,
                    name,
                    last4: None,
                    currency: remote.currency.clone(),
                    color: "#C9F950".to_string(),
                    opening_balance_cents: 0,
                    source: "simplefin".to_string(),
                    liquidity_type: "liquid".to_string(),
                    emergency_fund_eligible: true,
                    goal_earmark: None,
                    apy_pct: None,
                    simplefin_account_id: Some(simplefin_id.clone()),
                    nickname: nickname.clone(),
                    connection_id: Some(connection_id.clone()),
                    institution_id: connection.org_id.clone(),
                    external_account_id: Some(simplefin_id.clone()),
                    official_name: Some(remote.name.clone()),
                    mask: None,
                    subtype: None,
                    account_group: remote.account_group.clone(),
                    available_balance_cents: None,
                    balance_date: None,
                    extra_json: None,
                    raw_json: None,
                    import_pending: false,
                    apr_pct: None,
                    min_payment_cents: None,
                    payoff_date: None,
                    limit_cents: None,
                    original_balance_cents: None,
                    started_at: None,
                };
                accounts::upsert_simplefin_account(conn, account)
            }
        })
        .await
        .map_err(AppError::from)?;
        created_ids.push(local_id.id);
    }
    // Release before the initial-sync loop (each sync takes its own lease).
    drop(create_lease);

    // Initial sync for each imported account.
    for (req, local_id) in accounts.iter().zip(created_ids.iter()) {
        if let Err(e) = sync_local_account(&db, local_id, req.connection_id.clone(), false).await {
            tracing::error!("Initial sync failed for {local_id}: {}", e.message);
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
    let db = state.api.db.clone();

    let account = run(&db, {
        let account_id = account_id.clone();
        move |conn| accounts::get_by_id(conn, &account_id)
    })
    .await
    .map_err(AppError::from)?;

    let connection_id = account.connection_id.ok_or_else(|| {
        AppError::new("simplefin.not_linked", "Account is not linked to SimpleFin")
    })?;
    let summary =
        sync_local_account(&db, &account_id, connection_id, account.import_pending).await?;

    Ok(SyncSummary {
        added: summary.added,
        updated: summary.updated,
        skipped: summary.skipped,
        queued_for_review: summary.queued_for_review,
    })
}

async fn sync_local_account(
    db: &finsight_core::Db,
    account_id: &str,
    connection_id: String,
    import_pending: bool,
) -> AppResult<SimpleFinImportSummaryWrapper> {
    // Snapshot the ledger epoch before the (seconds-long, network) fetch. A
    // SimpleFin sync inserts top-level transaction/account rows with no FK to
    // guard them, so — unlike a categorization UPDATE that hits zero rows after
    // a wipe — a post-wipe sync commit would SURVIVE. We hold a reset lease
    // across the commit below and skip it if a Delete-All has landed, so a sync
    // in flight when Delete-All reports success can never commit afterward.
    let start_epoch = db.reset_barrier().epoch();

    let account = run(db, {
        let account_id = account_id.to_string();
        move |conn| accounts::get_by_id(conn, &account_id)
    })
    .await
    .map_err(AppError::from)?;

    let simplefin_id = account.simplefin_account_id.ok_or_else(|| {
        AppError::new(
            "simplefin.not_linked",
            "Account is missing SimpleFin account id",
        )
    })?;

    let connection = run(db, {
        let connection_id = connection_id.clone();
        move |conn| connections::get(conn, &connection_id)
    })
    .await
    .map_err(AppError::from)?;

    let access_url = keychain::get_key(SIMPLEFIN_ACCESS_SERVICE, &connection.access_url_ref)
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new(
                "simplefin.not_configured",
                "Access URL missing for this connection",
            )
        })?;

    let pending = match fetch_simplefin_data(
        &access_url,
        &simplefin_id,
        account_id,
        account.last_synced_at,
        import_pending,
    )
    .await
    {
        Ok(pending) => pending,
        Err(e) => {
            let app_error = AppError::from(e);
            let _ = mark_connection_error(db, &connection_id, app_error.message.clone()).await;
            return Err(app_error);
        }
    };

    // Hold a reset lease across the commit and skip if a Delete-All landed while
    // we fetched. The wipe drains this lease before running, so the fetched rows
    // either commit before the wipe (and are wiped) or are never written.
    let commit_lease = db.reset_barrier().writer_lease(start_epoch).await;
    if commit_lease.superseded() {
        return Err(AppError::new(
            "reset",
            "Sync cancelled: all data was cleared during the sync.",
        ));
    }
    let summary = match run(db, move |conn| {
        commit_simplefin_import(pending, conn)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    {
        Ok(summary) => summary,
        Err(e) => {
            let app_error = AppError::from(e);
            let _ = mark_connection_error(db, &connection_id, app_error.message.clone()).await;
            return Err(app_error);
        }
    };
    drop(commit_lease);

    // Clear any error status on success.
    let _ = run(db, {
        let connection_id = connection_id.clone();
        move |conn| {
            connections::update(
                conn,
                &connection_id,
                SimpleFinConnectionPatch {
                    status: Some("active".to_string()),
                    last_error: Some(None),
                    last_synced_at: Some(Some(Utc::now())),
                    ..Default::default()
                },
            )
        }
    })
    .await;

    Ok(SimpleFinImportSummaryWrapper {
        added: summary.added,
        updated: summary.updated,
        skipped: summary.skipped,
        queued_for_review: summary.queued_for_review,
    })
}

struct SimpleFinImportSummaryWrapper {
    added: usize,
    updated: usize,
    skipped: usize,
    queued_for_review: usize,
}

#[tauri::command]
#[specta::specta]
pub async fn disconnect_simplefin(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = state.api.db.clone();
    let bridge_ids = run(&db, |conn| {
        let mut stmt = conn.prepare("SELECT DISTINCT access_url_ref FROM simplefin_connections")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for id in rows {
            ids.push(id?);
        }
        Ok::<_, finsight_core::CoreError>(ids)
    })
    .await
    .map_err(AppError::from)?;

    for id in bridge_ids {
        keychain::delete_key(SIMPLEFIN_ACCESS_SERVICE, &id).map_err(AppError::from)?;
    }

    run(&db, |conn| {
        conn.execute("DELETE FROM simplefin_connections", [])?;
        conn.execute(
            "UPDATE accounts SET simplefin_account_id = NULL, connection_id = NULL, external_account_id = NULL WHERE source = 'simplefin'",
            [],
        )?;
        Ok::<_, finsight_core::CoreError>(())
    })
    .await
    .map_err(AppError::from)?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn purge_simplefin_data(
    state: tauri::State<'_, AppState>,
) -> AppResult<SimpleFinPurgeSummary> {
    let db = state.api.db.clone();
    let bridge_ids = run(&db, |conn| {
        let mut stmt = conn.prepare("SELECT DISTINCT access_url_ref FROM simplefin_connections")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for id in rows {
            ids.push(id?);
        }
        Ok::<_, finsight_core::CoreError>(ids)
    })
    .await
    .map_err(AppError::from)?;

    for id in bridge_ids {
        keychain::delete_key(SIMPLEFIN_ACCESS_SERVICE, &id).map_err(AppError::from)?;
    }

    run(&db, |conn| {
        let tx = conn.transaction()?;
        tx.execute_batch(
            "
            DROP TABLE IF EXISTS temp.purge_accounts;
            CREATE TEMP TABLE purge_accounts(id TEXT PRIMARY KEY);
            INSERT INTO purge_accounts(id)
            SELECT id FROM accounts WHERE source IN ('simplefin', 'sample');
            DROP TABLE IF EXISTS temp.purge_transactions;
            CREATE TEMP TABLE purge_transactions(id TEXT PRIMARY KEY);
            INSERT INTO purge_transactions(id)
            SELECT id FROM transactions
            WHERE account_id IN (SELECT id FROM purge_accounts)
               OR source IN ('simplefin', 'sample');
            ",
        )?;
        let accounts_deleted: i64 =
            tx.query_row("SELECT COUNT(*) FROM purge_accounts", [], |r| r.get(0))?;
        let transactions_deleted: i64 =
            tx.query_row("SELECT COUNT(*) FROM purge_transactions", [], |r| r.get(0))?;
        let connections_deleted: i64 =
            tx.query_row("SELECT COUNT(*) FROM simplefin_connections", [], |r| {
                r.get(0)
            })?;

        tx.execute_batch(
            "
            DELETE FROM transaction_transfers
            WHERE from_transaction_id IN (SELECT id FROM purge_transactions)
               OR to_transaction_id IN (SELECT id FROM purge_transactions);
            DELETE FROM transaction_splits
            WHERE txn_id IN (SELECT id FROM purge_transactions);
            DELETE FROM categorizations
            WHERE txn_id IN (SELECT id FROM purge_transactions);
            DELETE FROM import_candidate_matches
            WHERE transaction_id IN (SELECT id FROM purge_transactions)
               OR candidate_id IN (
                    SELECT id FROM import_candidates
                    WHERE account_id IN (SELECT id FROM purge_accounts)
                       OR source = 'simplefin'
               );
            DELETE FROM import_candidates
            WHERE account_id IN (SELECT id FROM purge_accounts)
               OR source = 'simplefin';
            DELETE FROM transactions
            WHERE id IN (SELECT id FROM purge_transactions);
            DELETE FROM account_balances
            WHERE account_id IN (SELECT id FROM purge_accounts);
            DELETE FROM simplefin_alerts
            WHERE account_id IN (SELECT id FROM purge_accounts);
            DELETE FROM holdings
            WHERE account_id IN (SELECT id FROM purge_accounts);
            DELETE FROM csv_import_mappings
            WHERE account_id IN (SELECT id FROM purge_accounts);
            UPDATE goals
            SET account_id = NULL
            WHERE account_id IN (SELECT id FROM purge_accounts);
            UPDATE planned_transactions
            SET account_id = NULL
            WHERE account_id IN (SELECT id FROM purge_accounts);
            UPDATE imports
            SET account_id = NULL
            WHERE account_id IN (SELECT id FROM purge_accounts);
            DELETE FROM imports
            WHERE source IN ('simplefin', 'sample');
            DELETE FROM accounts
            WHERE id IN (SELECT id FROM purge_accounts);
            DELETE FROM securities;
            ",
        )?;
        tx.execute("DELETE FROM simplefin_connections", [])?;
        settings::set(&tx, ONBOARDING_COMPLETION_KEY, &false)?;
        tx.commit()?;

        Ok::<_, finsight_core::CoreError>(SimpleFinPurgeSummary {
            accounts_deleted,
            transactions_deleted,
            connections_deleted,
        })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_simplefin_connection(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> AppResult<()> {
    let db = state.api.db.clone();
    let bridge_id = run(&db, {
        let connection_id = connection_id.clone();
        move |conn| {
            let c = connections::get(conn, &connection_id)?;
            Ok::<_, finsight_core::CoreError>(c.access_url_ref)
        }
    })
    .await
    .map_err(AppError::from)?;

    // Only delete the keychain entry if no other connection references it.
    let remaining = run(&db, {
        let bridge_id = bridge_id.clone();
        move |conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM simplefin_connections WHERE access_url_ref = ?1",
                [bridge_id],
                |r| r.get(0),
            )?;
            Ok::<_, finsight_core::CoreError>(count)
        }
    })
    .await
    .map_err(AppError::from)?;

    run(&db, {
        let connection_id = connection_id.clone();
        move |conn| {
            connections::delete(conn, &connection_id)?;
            conn.execute(
                "UPDATE accounts SET simplefin_account_id = NULL, connection_id = NULL, external_account_id = NULL WHERE connection_id = ?1",
                [&connection_id],
            )?;
            Ok::<_, finsight_core::CoreError>(())
        }
    })
    .await
    .map_err(AppError::from)?;

    if remaining <= 1 {
        keychain::delete_key(SIMPLEFIN_ACCESS_SERVICE, &bridge_id).map_err(AppError::from)?;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn sync_all_simplefin_accounts(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<crate::sync_scheduler::AccountSyncResult>> {
    let results = state.sync_scheduler.sync_all_now().await;
    Ok(results)
}

#[tauri::command]
#[specta::specta]
pub async fn get_simplefin_sync_settings(
    state: tauri::State<'_, AppState>,
) -> AppResult<crate::sync_scheduler::SimpleFinSyncSettings> {
    let interval = state.sync_scheduler.interval();
    let enabled = state.sync_scheduler.enabled();
    Ok(crate::sync_scheduler::SimpleFinSyncSettings {
        background_sync_enabled: enabled,
        background_sync_interval_minutes: interval,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn set_simplefin_sync_settings(
    state: tauri::State<'_, AppState>,
    settings: crate::sync_scheduler::SimpleFinSyncSettings,
) -> AppResult<()> {
    state
        .sync_scheduler
        .set_interval(settings.background_sync_interval_minutes);
    state
        .sync_scheduler
        .set_enabled(settings.background_sync_enabled);
    let db = state.api.db.clone();
    run(&db, move |conn| {
        finsight_core::settings::set(
            conn,
            "simplefin.background_sync_enabled",
            &settings.background_sync_enabled.to_string(),
        )?;
        finsight_core::settings::set(
            conn,
            "simplefin.background_sync_interval_minutes",
            &settings.background_sync_interval_minutes.to_string(),
        )
        .map(|_| ())
    })
    .await
    .map_err(AppError::from)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_alerts(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SimpleFinAlert>> {
    let db = state.api.db.clone();
    let rows = run(&db, |conn| alerts::list_unacknowledged(conn))
        .await
        .map_err(AppError::from)?;
    Ok(rows)
}

#[tauri::command]
#[specta::specta]
pub async fn acknowledge_simplefin_alert(
    state: tauri::State<'_, AppState>,
    alert_id: String,
) -> AppResult<()> {
    let db = state.api.db.clone();
    run(&db, {
        let alert_id = alert_id.clone();
        move |conn| alerts::acknowledge(conn, &alert_id)
    })
    .await
    .map_err(AppError::from)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_simplefin_transfer_suggestions(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<TransferSuggestionInfo>> {
    let db = state.api.db.clone();
    let rows = run(&db, |conn| transfers::list_suggestions(conn))
        .await
        .map_err(AppError::from)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn confirm_simplefin_transfer(
    state: tauri::State<'_, AppState>,
    transfer_id: String,
) -> AppResult<()> {
    let db = state.api.db.clone();
    run(&db, {
        let transfer_id = transfer_id.clone();
        move |conn| transfers::confirm(conn, &transfer_id)
    })
    .await
    .map_err(AppError::from)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn reject_simplefin_transfer(
    state: tauri::State<'_, AppState>,
    transfer_id: String,
) -> AppResult<()> {
    let db = state.api.db.clone();
    run(&db, {
        let transfer_id = transfer_id.clone();
        move |conn| transfers::reject(conn, &transfer_id)
    })
    .await
    .map_err(AppError::from)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_import_review_candidates(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<ImportCandidateWithMatches>> {
    let db = state.api.db.clone();
    run(&db, |conn| import_candidates::list_pending(conn))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn accept_import_candidate_match(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    transaction_id: String,
) -> AppResult<()> {
    let db = state.api.db.clone();
    run(&db, move |conn| {
        import_candidates::resolve_with_match(conn, &candidate_id, &transaction_id)
    })
    .await
    .map_err(AppError::from)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn create_import_candidate_transaction(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
) -> AppResult<String> {
    let db = state.api.db.clone();
    run(&db, move |conn| {
        import_candidates::resolve_create_new(conn, &candidate_id)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn dismiss_import_candidate(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
) -> AppResult<()> {
    let db = state.api.db.clone();
    run(&db, move |conn| {
        import_candidates::dismiss(conn, &candidate_id)
    })
    .await
    .map_err(AppError::from)?;
    Ok(())
}

fn upsert_institution_from_connection(
    conn: &mut rusqlite::Connection,
    pc: &ProviderConnection,
) -> Result<InstitutionModel, finsight_core::CoreError> {
    let id = pc.org_id.clone();
    institutions::upsert(
        conn,
        NewInstitution {
            id,
            name: pc.name.clone(),
            domain: pc.org_url.as_ref().and_then(extract_domain),
            sfin_url: Some(pc.sfin_url.clone()),
        },
    )
}

fn extract_domain(url: &String) -> Option<String> {
    // Very light extraction: strip scheme and path, keep host.
    let without_scheme = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    without_scheme
        .split('/')
        .next()
        .and_then(|host_port| host_port.split(':').next())
        .map(|s| s.to_string())
}

fn normalize_nickname(nickname: Option<&str>) -> Option<String> {
    nickname
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

async fn mark_connection_error(
    db: &finsight_core::Db,
    connection_id: &str,
    message: String,
) -> AppResult<()> {
    let connection_id = connection_id.to_string();
    run(db, move |conn| {
        connections::update(
            conn,
            &connection_id,
            SimpleFinConnectionPatch {
                status: Some("error".to_string()),
                last_error: Some(Some(message)),
                ..Default::default()
            },
        )
        .map(|_| ())
    })
    .await
    .map_err(AppError::from)
}
