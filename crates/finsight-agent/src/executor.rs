use finsight_core::{
    error::{CoreError, CoreResult},
    models::{AgentActionItem, NewPlannedTransaction, NewRule},
    repos::{agent_memory, budgets, copilot_actions, planned_transactions, rules, scenarios},
};
use rusqlite::{params, Connection};
use serde::Deserialize;

pub struct ExecutionResult {
    pub item_id: String,
    pub action_kind: String,
    pub status: String,
    pub result_summary: Option<String>,
    pub error: Option<String>,
}

pub struct BundleExecutionResult {
    pub bundle_id: String,
    pub executed: Vec<ExecutionResult>,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetBudgetPayload {
    category_id: String,
    month: String,
    amount_cents: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateGoalMonthlyPayload {
    goal_id: String,
    monthly_delta_cents: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateGoalTargetPayload {
    goal_id: String,
    target_cents: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetTransactionCategoryPayload {
    transaction_id: String,
    category_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetTransactionFlagPayload {
    transaction_id: String,
    flag: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRulePayload {
    pattern: String,
    category_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveScenarioPayload {
    description: String,
    params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateReportPayload {
    report_type: String,
    scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePlannedTransactionPayload {
    description: String,
    amount_cents: i64,
    due_date: String,
    account_id: Option<String>,
    category_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecategorizeBulkPayload {
    assignments: Vec<RecategorizeAssignment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecategorizeAssignment {
    transaction_id: String,
    category_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebtPayoffPlanPayload {
    method: String,
    extra_monthly_cents: i64,
    account_ids: Option<Vec<String>>,
}

pub fn execute_bundle(conn: &mut Connection, bundle_id: &str) -> CoreResult<BundleExecutionResult> {
    let Some(bundle) = copilot_actions::get_bundle(conn, bundle_id)? else {
        return Err(CoreError::InvalidState(format!(
            "Unknown bundle id: {bundle_id}"
        )));
    };
    if !matches!(bundle.status.as_str(), "pending" | "reviewed") {
        return Err(CoreError::InvalidState(format!(
            "Bundle {} cannot be executed from status {}",
            bundle.id, bundle.status
        )));
    }

    copilot_actions::set_bundle_status(conn, bundle_id, "executing")?;

    let approved_items: Vec<_> = bundle
        .items
        .into_iter()
        .filter(|item| item.status == "approved")
        .collect();

    let mut executed = Vec::new();
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for item in approved_items {
        match execute_item(conn, &item) {
            Ok(summary) => {
                copilot_actions::set_item_status(conn, &item.id, "executed")?;
                let result_json = serde_json::json!({ "summary": summary }).to_string();
                copilot_actions::insert_execution_log_entry(
                    conn,
                    &item.id,
                    bundle_id,
                    &item.action_kind,
                    "success",
                    Some(&result_json),
                    None,
                )?;
                succeeded += 1;
                executed.push(ExecutionResult {
                    item_id: item.id,
                    action_kind: item.action_kind,
                    status: "success".into(),
                    result_summary: Some(summary),
                    error: None,
                });
            }
            Err(err) => {
                let status = if err.to_string().contains("validation:") {
                    "validation_error"
                } else {
                    "failed"
                };
                copilot_actions::set_item_status(conn, &item.id, "failed")?;
                copilot_actions::insert_execution_log_entry(
                    conn,
                    &item.id,
                    bundle_id,
                    &item.action_kind,
                    status,
                    None,
                    Some(&err.to_string()),
                )?;
                failed += 1;
                executed.push(ExecutionResult {
                    item_id: item.id,
                    action_kind: item.action_kind,
                    status: status.into(),
                    result_summary: None,
                    error: Some(err.to_string()),
                });
            }
        }
    }

    let final_status = if failed == 0 {
        "executed"
    } else if succeeded == 0 {
        "failed"
    } else {
        "partially_executed"
    };
    copilot_actions::set_bundle_status(conn, bundle_id, final_status)?;

    Ok(BundleExecutionResult {
        bundle_id: bundle_id.to_string(),
        executed,
        succeeded,
        failed,
    })
}

fn execute_item(conn: &mut Connection, item: &AgentActionItem) -> CoreResult<String> {
    match item.action_kind.as_str() {
        "set_budget" => {
            let payload: SetBudgetPayload = parse_payload(&item.payload_json)?;
            budgets::set(
                conn,
                &payload.category_id,
                &payload.month,
                payload.amount_cents,
            )?;
            Ok(format!(
                "Budget for {} in {} set to ${:.0}",
                payload.category_id,
                payload.month,
                payload.amount_cents as f64 / 100.0
            ))
        }
        "update_goal_monthly" => {
            let payload: UpdateGoalMonthlyPayload = parse_payload(&item.payload_json)?;
            let current: i64 = conn.query_row(
                "SELECT monthly_cents FROM goals WHERE id = ?1",
                params![payload.goal_id],
                |r| r.get(0),
            )?;
            let new_monthly = (current + payload.monthly_delta_cents).max(0);
            let changed = conn.execute(
                "UPDATE goals SET monthly_cents = ?1 WHERE id = ?2",
                params![new_monthly, payload.goal_id],
            )?;
            ensure_changed(changed, "goal")?;
            Ok(format!(
                "Goal {} monthly contribution updated to ${:.0}",
                payload.goal_id,
                new_monthly as f64 / 100.0
            ))
        }
        "update_goal_target" => {
            let payload: UpdateGoalTargetPayload = parse_payload(&item.payload_json)?;
            let changed = conn.execute(
                "UPDATE goals SET target_cents = ?1 WHERE id = ?2",
                params![payload.target_cents, payload.goal_id],
            )?;
            ensure_changed(changed, "goal")?;
            Ok(format!(
                "Goal {} target updated to ${:.0}",
                payload.goal_id,
                payload.target_cents as f64 / 100.0
            ))
        }
        "set_transaction_category" => {
            let payload: SetTransactionCategoryPayload = parse_payload(&item.payload_json)?;
            let changed = conn.execute(
                "UPDATE transactions
                 SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL
                 WHERE id = ?2",
                params![payload.category_id, payload.transaction_id],
            )?;
            ensure_changed(changed, "transaction")?;
            let (merchant_raw, category_label): (String, String) = conn.query_row(
                "SELECT t.merchant_raw, COALESCE(c.label, ?3)
                 FROM transactions t
                 LEFT JOIN categories c ON c.id = ?1
                 WHERE t.id = ?2",
                params![
                    payload.category_id,
                    payload.transaction_id,
                    payload.category_id
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let memo = format!("{merchant_raw} → {category_label} (agent correction)");
            agent_memory::upsert_correction(conn, &merchant_raw.to_lowercase(), &memo)?;
            Ok(format!(
                "Transaction {} categorized as {}",
                payload.transaction_id, category_label
            ))
        }
        "set_transaction_flag" => {
            let payload: SetTransactionFlagPayload = parse_payload(&item.payload_json)?;
            let (sql, summary) = match payload.flag.as_str() {
                "reimbursable" => (
                    "UPDATE transactions SET is_reimbursable = 1 WHERE id = ?1",
                    format!("Transaction {} marked reimbursable", payload.transaction_id),
                ),
                "anomaly_clear" => (
                    "UPDATE transactions SET is_anomaly = 0 WHERE id = ?1",
                    format!("Transaction {} anomaly cleared", payload.transaction_id),
                ),
                other => {
                    return Err(CoreError::InvalidState(format!(
                        "validation: unsupported transaction flag '{other}'"
                    )))
                }
            };
            let changed = conn.execute(sql, params![payload.transaction_id])?;
            ensure_changed(changed, "transaction")?;
            Ok(summary)
        }
        "create_rule" => {
            let payload: CreateRulePayload = parse_payload(&item.payload_json)?;
            let rule = rules::insert(
                conn,
                NewRule {
                    pattern: payload.pattern.clone(),
                    category_id: payload.category_id.clone(),
                    source: "agent".into(),
                },
            )?;
            Ok(format!(
                "Rule {} created for pattern '{}'",
                rule.id, payload.pattern
            ))
        }
        "save_scenario" => {
            let payload: SaveScenarioPayload = parse_payload(&item.payload_json)?;
            let row = scenarios::insert(
                conn,
                &payload.description,
                &serde_json::to_string(&payload.params)
                    .map_err(|e| CoreError::InvalidState(e.to_string()))?,
            )?;
            Ok(format!(
                "Scenario '{}' saved as {}",
                row.description, row.id
            ))
        }
        "generate_report" => {
            let payload: GenerateReportPayload = parse_payload(&item.payload_json)?;
            Ok(format!(
                "Report generation acknowledged for {} ({}) — view in Reports screen",
                payload.report_type, payload.scope
            ))
        }

        "create_planned_transaction" => {
            let payload: CreatePlannedTransactionPayload = parse_payload(&item.payload_json)?;
            let row = planned_transactions::insert(
                conn,
                NewPlannedTransaction {
                    description: payload.description.clone(),
                    amount_cents: payload.amount_cents,
                    account_id: payload.account_id,
                    category_id: payload.category_id,
                    due_date: payload.due_date.clone(),
                    source: "agent".to_string(),
                },
            )?;
            Ok(format!(
                "Planned transaction '{}' saved for {} as {}",
                payload.description, payload.due_date, row.id
            ))
        }
        "recategorize_bulk" => {
            let payload: RecategorizeBulkPayload = parse_payload(&item.payload_json)?;
            if payload.assignments.is_empty() {
                return Err(CoreError::InvalidState(
                    "validation: recategorize_bulk has no assignments".to_string(),
                ));
            }
            let mut applied = 0usize;
            let mut skipped = 0usize;
            for a in &payload.assignments {
                // Category must still exist and be active.
                let cat_ok: bool = conn
                    .query_row(
                        "SELECT 1 FROM categories WHERE id = ?1 AND archived_at IS NULL",
                        params![a.category_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);
                if !cat_ok {
                    skipped += 1;
                    continue;
                }
                // EXECUTE-TIME GUARD: only apply when the transaction is STILL
                // uncategorized. If the user categorized it manually between the
                // preview and now, do not clobber their choice.
                let changed = conn.execute(
                    "UPDATE transactions
                     SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL
                     WHERE id = ?2 AND category_id IS NULL",
                    params![a.category_id, a.transaction_id],
                )?;
                if changed == 0 {
                    skipped += 1;
                    continue;
                }
                // Record the categorization + a correction memory so the built-in
                // categorizer learns this merchant mapping going forward.
                if let Ok((merchant_raw, category_label)) = conn.query_row(
                    "SELECT t.merchant_raw, COALESCE(c.label, ?2)
                     FROM transactions t LEFT JOIN categories c ON c.id = ?1
                     WHERE t.id = ?3",
                    params![a.category_id, a.category_id, a.transaction_id],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                ) {
                    let memo = format!("{merchant_raw} → {category_label} (agent recategorization)");
                    let _ = agent_memory::upsert_correction(
                        conn,
                        &merchant_raw.to_lowercase(),
                        &memo,
                    );
                }
                applied += 1;
            }
            Ok(format!(
                "Recategorized {applied} transaction(s); skipped {skipped} (already categorized or invalid)"
            ))
        }
        "debt_payoff_plan" => {
            let payload: DebtPayoffPlanPayload = parse_payload(&item.payload_json)?;
            let tracked = payload
                .account_ids
                .as_ref()
                .map(|ids| ids.len())
                .unwrap_or(0);
            Ok(format!(
                "Debt payoff plan acknowledged: {} method, ${:.0}/mo extra, {} targeted debt(s)",
                payload.method,
                payload.extra_monthly_cents as f64 / 100.0,
                tracked
            ))
        }
        other => Err(CoreError::InvalidState(format!(
            "Unknown action kind: {other}"
        ))),
    }
}

fn parse_payload<T: for<'de> Deserialize<'de>>(json: &str) -> CoreResult<T> {
    serde_json::from_str(json)
        .map_err(|e| CoreError::InvalidState(format!("validation: invalid payload: {e}")))
}

fn ensure_changed(changed: usize, entity: &str) -> CoreResult<()> {
    if changed == 0 {
        Err(CoreError::InvalidState(format!(
            "validation: {entity} not found"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use finsight_core::{
        db::run_migrations,
        keychain,
        models::{AccountType, NewAccount, NewTransaction, TransactionStatus},
        repos::{accounts, copilot_actions, goals, transactions},
        Db,
    };
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("executor.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_category(conn: &mut Connection) {
        conn.execute(
            "INSERT INTO category_groups(id, label, sort_order) VALUES('grp1', 'Core', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES('cat1', 'grp1', 'Groceries', '#00ff00', 0)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn execute_bundle_applies_approved_actions() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn);
        let account = accounts::insert(
            &mut conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Checking".into(),
                last4: None,
                currency: "USD".into(),
                color: "#112233".into(),
                opening_balance_cents: 200_000,
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
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
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                limit_cents: None,
                original_balance_cents: None,
                started_at: None,
            },
        )
        .unwrap();
        let goal = goals::insert(
            &mut conn,
            goals::NewGoal {
                name: "Trip".into(),
                goal_type: "save-by-date".into(),
                target_cents: 500_000,
                monthly_cents: 20_000,
                target_date: None,
                color: "#abcdef".into(),
                notes: None,
                purpose: None,
                account_id: None,
            },
        )
        .unwrap();
        let txn = transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: account.id.clone(),
                posted_at: Utc::now() - Duration::days(1),
                amount_cents: -12_345,
                merchant_raw: "Whole Foods".into(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
            },
        )
        .unwrap();
        conn.execute(
            "UPDATE transactions SET is_anomaly = 1 WHERE id = ?1",
            params![txn.id],
        )
        .unwrap();

        let bundle = copilot_actions::insert_bundle(
            &mut conn,
            None,
            "Agent updates",
            "Summary",
            "Rationale",
            0.9,
            Some("provider"),
            Some("model"),
        )
        .unwrap();
        let items = [
            (
                "set_budget",
                serde_json::json!({"categoryId":"cat1","month":"2026-06","amountCents":45000}),
            ),
            (
                "update_goal_monthly",
                serde_json::json!({"goalId":goal.id,"monthlyDeltaCents":5000}),
            ),
            (
                "set_transaction_category",
                serde_json::json!({"transactionId":txn.id,"categoryId":"cat1"}),
            ),
            (
                "set_transaction_flag",
                serde_json::json!({"transactionId":txn.id,"flag":"anomaly_clear"}),
            ),
        ];
        for (idx, (kind, payload)) in items.into_iter().enumerate() {
            let item = copilot_actions::insert_item(
                &mut conn,
                &bundle.id,
                kind,
                &payload.to_string(),
                "Rationale",
                0.8,
                idx as i64,
            )
            .unwrap();
            copilot_actions::set_item_status(&mut conn, &item.id, "approved").unwrap();
        }

        let result = execute_bundle(&mut conn, &bundle.id).unwrap();

        assert_eq!(result.succeeded, 4);
        assert_eq!(result.failed, 0);
        let budget_amount: i64 = conn
            .query_row(
                "SELECT amount_cents FROM budgets WHERE category_id = 'cat1' AND month = '2026-06'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(budget_amount, 45_000);
        let updated_monthly: i64 = conn
            .query_row(
                "SELECT monthly_cents FROM goals WHERE id = ?1",
                params![goal.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(updated_monthly, 25_000);
        let (category_id, is_anomaly): (Option<String>, i64) = conn
            .query_row(
                "SELECT category_id, is_anomaly FROM transactions WHERE id = ?1",
                params![txn.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(category_id.as_deref(), Some("cat1"));
        assert_eq!(is_anomaly, 0);
        let memory_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM agent_memory", [], |r| r.get(0))
            .unwrap();
        assert_eq!(memory_count, 1);
    }

    #[test]
    fn recategorize_bulk_applies_only_still_uncategorized() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn);
        // Second category to prove the guard, not category identity, is what skips.
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES('cat2', 'grp1', 'Dining', '#ff0000', 1)",
            [],
        )
        .unwrap();
        let account = accounts::insert(&mut conn, base_account()).unwrap();

        let mk = |conn: &mut Connection, merchant: &str| {
            transactions::insert(
                conn,
                NewTransaction {
                    account_id: account.id.clone(),
                    posted_at: Utc::now() - Duration::days(2),
                    amount_cents: -5_000,
                    merchant_raw: merchant.into(),
                    category_id: None,
                    notes: None,
                    status: TransactionStatus::Cleared,
                    imported_id: None,
                    source: None,
                    raw_synced_data: None,
                    pending: false,
                    external_tx_id: None,
                    external_account_id: None,
                },
            )
            .unwrap()
            .id
        };
        let t_uncat = mk(&mut conn, "Mystery Merchant");
        let t_already = mk(&mut conn, "Already Categorized");
        // Simulate the user categorizing t_already AFTER the preview was drafted.
        conn.execute(
            "UPDATE transactions SET category_id = 'cat2' WHERE id = ?1",
            params![t_already],
        )
        .unwrap();

        let bundle = copilot_actions::insert_bundle(
            &mut conn, None, "Recat", "Summary", "Rationale", 0.8, None, None,
        )
        .unwrap();
        let payload = serde_json::json!({
            "assignments": [
                {"transactionId": t_uncat, "categoryId": "cat1"},
                {"transactionId": t_already, "categoryId": "cat1"},
                {"transactionId": "nonexistent", "categoryId": "cat1"},
                {"transactionId": t_uncat, "categoryId": "ghost-category"},
            ]
        });
        let item = copilot_actions::insert_item(
            &mut conn,
            &bundle.id,
            "recategorize_bulk",
            &payload.to_string(),
            "Recategorize",
            0.8,
            0,
        )
        .unwrap();
        copilot_actions::set_item_status(&mut conn, &item.id, "approved").unwrap();

        let result = execute_bundle(&mut conn, &bundle.id).unwrap();
        assert_eq!(result.succeeded, 1);
        assert!(result.executed[0]
            .result_summary
            .as_ref()
            .unwrap()
            .contains("Recategorized 1"));

        // Only the still-uncategorized txn was updated.
        let uncat_cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id = ?1",
                params![t_uncat],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(uncat_cat.as_deref(), Some("cat1"));
        // The manually-categorized txn was NOT clobbered.
        let already_cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id = ?1",
                params![t_already],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(already_cat.as_deref(), Some("cat2"), "manual category preserved");
    }

    fn base_account() -> NewAccount {
        NewAccount {
            owner: "Me".into(),
            bank: "Bank".into(),
            r#type: AccountType::Checking,
            name: "Checking".into(),
            last4: None,
            currency: "USD".into(),
            color: "#112233".into(),
            opening_balance_cents: 200_000,
            source: "manual".into(),
            liquidity_type: "liquid".into(),
            emergency_fund_eligible: true,
            goal_earmark: None,
            apy_pct: None,
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
            apr_pct: None,
            min_payment_cents: None,
            payoff_date: None,
            limit_cents: None,
            original_balance_cents: None,
            started_at: None,
        }
    }

    #[test]
    fn execute_bundle_marks_validation_errors_failed() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let bundle = copilot_actions::insert_bundle(
            &mut conn,
            None,
            "Broken bundle",
            "Summary",
            "Rationale",
            0.5,
            None,
            None,
        )
        .unwrap();
        let item = copilot_actions::insert_item(
            &mut conn,
            &bundle.id,
            "set_transaction_flag",
            r#"{"transactionId":"missing","flag":"unsupported"}"#,
            "Bad flag",
            0.4,
            0,
        )
        .unwrap();
        copilot_actions::set_item_status(&mut conn, &item.id, "approved").unwrap();

        let result = execute_bundle(&mut conn, &bundle.id).unwrap();

        assert_eq!(result.succeeded, 0);
        assert_eq!(result.failed, 1);
        assert_eq!(result.executed[0].status, "validation_error");
        let stored = copilot_actions::get_bundle(&mut conn, &bundle.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, "failed");
        assert_eq!(stored.items[0].status, "failed");
    }
}
