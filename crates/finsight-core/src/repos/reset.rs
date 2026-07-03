use crate::error::CoreResult;
use rusqlite::Connection;

/// Every user-data table wiped by a full factory reset. Deliberately excludes
/// `settings` — provider config (selected LLM provider/model, currency,
/// notification/auto-categorize toggles, onboarding-completion flag) is app
/// configuration, not financial data, and API keys live in the OS keychain
/// (never in SQLite), so neither needs to be touched here.
const TABLES_TO_WIPE: &[&str] = &[
    "account_balances",
    "accounts",
    "agent_action_bundles",
    "agent_action_items",
    "agent_context_snapshots",
    "agent_execution_log",
    "agent_memory",
    "agent_recipe_runs",
    "agent_recipes",
    "agent_sessions",
    "audit_log",
    "budgets",
    "categories",
    "categorizations",
    "category_groups",
    "conversation_messages",
    "conversations",
    "csv_import_mappings",
    "goals",
    "holdings",
    "import_candidate_matches",
    "import_candidates",
    "imports",
    "institutions",
    "liabilities",
    "manual_assets",
    "merchants",
    "monthly_reviews",
    "net_worth_milestones",
    "net_worth_snapshots",
    "planned_transactions",
    "rule_proposals",
    "rules",
    "scenarios",
    "securities",
    "simplefin_alerts",
    "simplefin_connections",
    "sync_runs",
    "transaction_splits",
    "transaction_transfers",
    "transactions",
];

/// Wipes every local financial/user-data table, leaving `settings` (and the
/// OS keychain, which this never touches) intact. Foreign keys are disabled
/// for the duration since deletion order across 40 interrelated tables would
/// otherwise have to respect FK dependency order exactly.
pub fn delete_all_data(conn: &mut Connection) -> CoreResult<()> {
    conn.pragma_update(None, "foreign_keys", false)?;
    let result = (|| -> CoreResult<()> {
        let tx = conn.transaction()?;
        for table in TABLES_TO_WIPE {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.commit()?;
        Ok(())
    })();
    conn.pragma_update(None, "foreign_keys", true)?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, models::NewAccount, repos::accounts, Db};
    use rusqlite::params;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("reset.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn wipes_accounts_transactions_and_settings_survive() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        // Seed a setting that must survive the wipe.
        crate::settings::set(&conn, "llm_provider", &"ollama").unwrap();

        let acct = accounts::insert(
            &mut conn,
            NewAccount {
                owner: "me".into(),
                bank: "Bank".into(),
                r#type: crate::models::AccountType::Checking,
                name: "Checking".into(),
                last4: None,
                currency: "USD".into(),
                color: "#000".into(),
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                opening_balance_cents: 10_000,
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "cash".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
            },
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, created_at) \
             VALUES(?1, ?2, ?3, -500, 'Coffee', 'cleared', ?3)",
            params![Uuid::new_v4().to_string(), acct.id, chrono::Utc::now().to_rfc3339()],
        )
        .unwrap();

        delete_all_data(&mut conn).unwrap();

        let acct_count: i64 = conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0)).unwrap();
        let txn_count: i64 = conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0)).unwrap();
        let balance_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM account_balances", [], |r| r.get(0))
            .unwrap();
        assert_eq!(acct_count, 0);
        assert_eq!(txn_count, 0);
        assert_eq!(balance_count, 0);

        let provider: Option<String> = crate::settings::get(&conn, "llm_provider").unwrap();
        assert_eq!(provider.as_deref(), Some("ollama"), "settings must survive a data wipe");
    }

    #[test]
    fn wipes_copilot_history_memory_and_context_so_no_stale_data_survives() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Seed Copilot conversation history, an agent action bundle + item,
        // agent memory, a cached context snapshot, and a net-worth snapshot —
        // exactly the kinds of stale data a reset must not leave behind.
        conn.execute(
            "INSERT INTO conversations(id, title, created_at, updated_at) VALUES('c1','Old chat',?1,?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversation_messages(id, conversation_id, role, content, created_at) \
             VALUES('m1','c1','assistant','Your net worth is $123,456',?1)",
            params![now],
        )
        .unwrap();
        let bundle = super::super::copilot_actions::insert_bundle(
            &mut conn, None, "Recat", "summary", "rationale", 0.9, None, None,
        )
        .unwrap();
        super::super::copilot_actions::insert_item(
            &mut conn,
            &bundle.id,
            "recategorize_bulk",
            "{}",
            "rationale",
            0.9,
            0,
        )
        .unwrap();
        super::super::agent_memory::upsert_correction(&mut conn, "cafe", "cafe -> Dining").unwrap();
        super::super::net_worth::record_snapshot(&mut conn, 5_000_000).unwrap();

        delete_all_data(&mut conn).unwrap();

        for table in [
            "conversations",
            "conversation_messages",
            "agent_action_bundles",
            "agent_action_items",
            "agent_memory",
            "agent_context_snapshots",
            "net_worth_snapshots",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0, "{table} must be empty after reset");
        }

        // Net worth is no longer meaningful with nothing tracked.
        assert!(!super::super::net_worth::breakdown(&mut conn).unwrap().has_data);
    }

    #[test]
    fn is_safe_to_run_on_an_already_empty_database() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        delete_all_data(&mut conn).unwrap();
        delete_all_data(&mut conn).unwrap();
    }
}
