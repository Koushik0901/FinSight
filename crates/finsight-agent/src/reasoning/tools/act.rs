use crate::reasoning::messages::{AgentChange, AgentDraftAction};
use crate::reasoning::tools::{Tool, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn set_budget() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "draft_set_budget"
        }
        fn description(&self) -> &str {
            "Draft a monthly budget amount change for user approval; does not write data"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"category_id":{"type":"string"},"month":{"type":"string"},"amount_cents":{"type":"integer"},"rationale":{"type":"string"}},"required":["category_id","month","amount_cents"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let category_id = args["category_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("category_id required"))?;
            let month = args["month"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("month required"))?;
            let amount_cents = args["amount_cents"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("amount_cents required"))?
                .max(0);
            let category_label = ctx
                .conn
                .query_row(
                    "SELECT label FROM categories WHERE id = ?1",
                    rusqlite::params![category_id],
                    |r| r.get::<_, String>(0),
                )
                .unwrap_or_else(|_| category_id.to_string());
            let rationale = args["rationale"]
                .as_str()
                .unwrap_or("Budget change recommended by finance analysis.")
                .to_string();
            let payload = json!({
                "categoryId": category_id,
                "month": month,
                "amountCents": amount_cents,
            });
            ctx.draft_actions.push(AgentDraftAction {
                action_kind: "set_budget".to_string(),
                payload_json: payload.to_string(),
                rationale: rationale.clone(),
                confidence: 0.8,
            });
            ctx.changes.push(AgentChange {
                kind: "draft_action".to_string(),
                description: format!(
                    "Drafted {} budget for {} at ${:.2}",
                    category_label,
                    month,
                    amount_cents as f64 / 100.0
                ),
            });
            Ok(json!({"drafted": true, "payload": payload}))
        }
    }
    Arc::new(T)
}

pub fn update_goal_monthly() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "draft_update_goal_monthly"
        }
        fn description(&self) -> &str {
            "Draft a goal monthly contribution change for user approval; does not write data"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"goal_id": {"type": "string"}, "monthly_delta_cents": {"type": "integer"}, "rationale": {"type": "string"}}, "required": ["goal_id", "monthly_delta_cents"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let goal_id = args["goal_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("goal_id required"))?;
            let delta = args["monthly_delta_cents"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("monthly_delta_cents required"))?;
            let (goal_name, old_monthly): (String, i64) = ctx.conn.query_row(
                "SELECT name, monthly_cents FROM goals WHERE id = ?1",
                rusqlite::params![goal_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let new_monthly = (old_monthly + delta).max(0);
            let rationale = args["rationale"]
                .as_str()
                .unwrap_or("Recommended by finance analysis.")
                .to_string();
            let payload = json!({"goalId": goal_id, "monthlyDeltaCents": delta});
            ctx.draft_actions.push(AgentDraftAction {
                action_kind: "update_goal_monthly".to_string(),
                payload_json: payload.to_string(),
                rationale: rationale.clone(),
                confidence: 0.85,
            });
            ctx.changes.push(AgentChange {
                kind: "draft_action".to_string(),
                description: format!(
                    "Drafted goal change for '{}': ${}/mo -> ${}/mo",
                    goal_name,
                    old_monthly / 100,
                    new_monthly / 100
                ),
            });
            Ok(
                json!({"drafted": true, "goal_id": goal_id, "old_monthly_cents": old_monthly, "new_monthly_cents": new_monthly, "payload": payload}),
            )
        }
    }
    Arc::new(T)
}

pub fn create_planned_transaction() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "draft_create_planned_transaction"
        }
        fn description(&self) -> &str {
            "Draft a future payment, transfer, or investment for user approval; does not write data"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {
                "description": {"type": "string"},
                "amount_cents": {"type": "integer"},
                "due_date": {"type": "string"},
                "account_id": {"type": "string"},
                "category_id": {"type": "string"},
                "rationale": {"type": "string"}
            }, "required": ["description", "amount_cents", "due_date"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let description = args["description"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("description required"))?
                .to_string();
            let amount = args["amount_cents"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("amount_cents required"))?;
            let due_date = args["due_date"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("due_date required"))?
                .to_string();
            let payload = json!({
                "description": description,
                "amountCents": amount,
                "dueDate": due_date,
                "accountId": args["account_id"].as_str(),
                "categoryId": args["category_id"].as_str(),
            });
            let rationale = args["rationale"]
                .as_str()
                .unwrap_or("Recommended by finance analysis.")
                .to_string();
            ctx.draft_actions.push(AgentDraftAction {
                action_kind: "create_planned_transaction".to_string(),
                payload_json: payload.to_string(),
                rationale: rationale.clone(),
                confidence: 0.8,
            });
            ctx.changes.push(AgentChange {
                kind: "draft_action".to_string(),
                description: format!(
                    "Drafted planned transaction '${:.2}' for '{}' on {}",
                    amount as f64 / 100.0,
                    description,
                    due_date
                ),
            });
            Ok(json!({"drafted": true, "payload": payload}))
        }
    }
    Arc::new(T)
}

pub fn save_scenario() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "draft_save_scenario"
        }
        fn description(&self) -> &str {
            "Draft saving a scenario for user approval"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"description":{"type":"string"},"params":{"type":"object"},"rationale":{"type":"string"}},"required":["description","params"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let description = args["description"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("description required"))?
                .to_string();
            let params = args.get("params").cloned().unwrap_or_else(|| json!({}));
            let payload = json!({"description": description, "params": params});
            ctx.draft_actions.push(AgentDraftAction {
                action_kind: "save_scenario".to_string(),
                payload_json: payload.to_string(),
                rationale: args["rationale"]
                    .as_str()
                    .unwrap_or("Save this what-if for later review.")
                    .to_string(),
                confidence: 0.8,
            });
            ctx.changes.push(AgentChange {
                kind: "draft_action".to_string(),
                description: "Drafted scenario save".to_string(),
            });
            Ok(json!({"drafted": true, "payload": payload}))
        }
    }
    Arc::new(T)
}

pub fn draft_recategorization() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "draft_recategorization"
        }
        fn description(&self) -> &str {
            "Draft a bulk recategorization of currently-uncategorized transactions for user approval. Provide assignments from list_uncategorized_transactions: each maps a transaction_id to a category_id (from available_categories) with a confidence 0..1. This does NOT write data — it previews the proposed changes; the user must approve before anything is applied. Invalid assignments (unknown category, or a transaction that is no longer uncategorized) are dropped and reported."
        }
        fn parameters(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "assignments": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "transaction_id": {"type": "string"},
                                "category_id": {"type": "string"},
                                "confidence": {"type": "number"}
                            },
                            "required": ["transaction_id", "category_id"]
                        }
                    }
                },
                "required": ["assignments"]
            })
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let raw = args["assignments"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("assignments array required"))?;
            if raw.is_empty() {
                return Err(anyhow::anyhow!("assignments must not be empty"));
            }

            // Cap the batch so the preview payload stays bounded.
            const MAX_ASSIGNMENTS: usize = 100;

            let mut valid: Vec<Value> = Vec::new();
            let mut dropped: Vec<Value> = Vec::new();
            let mut seen_txns = std::collections::HashSet::new();

            for a in raw.iter().take(MAX_ASSIGNMENTS) {
                let (Some(txn_id), Some(cat_id)) =
                    (a["transaction_id"].as_str(), a["category_id"].as_str())
                else {
                    dropped.push(json!({"reason": "missing transaction_id or category_id"}));
                    continue;
                };
                if !seen_txns.insert(txn_id.to_string()) {
                    dropped.push(json!({"transaction_id": txn_id, "reason": "duplicate"}));
                    continue;
                }
                // Category must exist and be active.
                let cat_label: Option<String> = ctx
                    .conn
                    .query_row(
                        "SELECT label FROM categories WHERE id = ?1 AND archived_at IS NULL",
                        rusqlite::params![cat_id],
                        |r| r.get(0),
                    )
                    .ok();
                let Some(cat_label) = cat_label else {
                    dropped.push(json!({"transaction_id": txn_id, "reason": "unknown category_id"}));
                    continue;
                };
                // Transaction must exist AND still be uncategorized.
                let merchant: Option<String> = ctx
                    .conn
                    .query_row(
                        "SELECT merchant_raw FROM transactions WHERE id = ?1 AND category_id IS NULL",
                        rusqlite::params![txn_id],
                        |r| r.get(0),
                    )
                    .ok();
                let Some(merchant) = merchant else {
                    dropped.push(json!({"transaction_id": txn_id, "reason": "transaction not found or already categorized"}));
                    continue;
                };
                let confidence = a["confidence"].as_f64().unwrap_or(0.7).clamp(0.0, 1.0);
                valid.push(json!({
                    "transactionId": txn_id,
                    "categoryId": cat_id,
                    "categoryLabel": cat_label,
                    "merchant": merchant,
                    "confidence": confidence
                }));
            }

            if valid.is_empty() {
                return Ok(json!({
                    "drafted": false,
                    "proposed": 0,
                    "dropped": dropped.len(),
                    "dropped_detail": dropped,
                    "message": "No valid recategorization assignments to preview."
                }));
            }

            let proposed = valid.len();
            let avg_conf = valid
                .iter()
                .filter_map(|v| v["confidence"].as_f64())
                .sum::<f64>()
                / proposed as f64;
            let preview_labels: Vec<String> = valid
                .iter()
                .take(5)
                .map(|v| {
                    format!(
                        "{} → {}",
                        v["merchant"].as_str().unwrap_or(""),
                        v["categoryLabel"].as_str().unwrap_or("")
                    )
                })
                .collect();
            let more = proposed.saturating_sub(preview_labels.len());
            let rationale = if more > 0 {
                format!(
                    "Recategorize {proposed} uncategorized transactions ({}, +{more} more).",
                    preview_labels.join(", ")
                )
            } else {
                format!(
                    "Recategorize {proposed} uncategorized transactions ({}).",
                    preview_labels.join(", ")
                )
            };

            let payload = json!({ "assignments": valid });
            ctx.draft_actions.push(AgentDraftAction {
                action_kind: "recategorize_bulk".to_string(),
                payload_json: payload.to_string(),
                rationale: rationale.clone(),
                confidence: avg_conf,
            });
            ctx.changes.push(AgentChange {
                kind: "draft_action".to_string(),
                description: rationale,
            });

            Ok(json!({
                "drafted": true,
                "proposed": proposed,
                "dropped": dropped.len(),
                "dropped_detail": dropped,
                "requires_approval": true
            }))
        }
    }
    Arc::new(T)
}

pub fn create_debt_payoff_plan() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "draft_debt_payoff_plan"
        }
        fn description(&self) -> &str {
            "Draft a debt payoff plan item for approval and tracking; does not mutate liabilities"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"method":{"type":"string"},"extra_monthly_cents":{"type":"integer"},"liability_ids":{"type":"array","items":{"type":"string"}},"rationale":{"type":"string"}},"required":["method"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let method = args["method"].as_str().unwrap_or("avalanche").to_string();
            let payload = json!({
                "method": method,
                "extraMonthlyCents": args["extra_monthly_cents"].as_i64().unwrap_or(0),
                "liabilityIds": args["liability_ids"].clone(),
            });
            ctx.draft_actions.push(AgentDraftAction {
                action_kind: "debt_payoff_plan".to_string(),
                payload_json: payload.to_string(),
                rationale: args["rationale"]
                    .as_str()
                    .unwrap_or("Debt payoff plan generated from finance analysis.")
                    .to_string(),
                confidence: 0.85,
            });
            ctx.changes.push(AgentChange {
                kind: "draft_action".to_string(),
                description: "Drafted debt payoff plan".to_string(),
            });
            Ok(json!({"drafted": true, "payload": payload}))
        }
    }
    Arc::new(T)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::messages::{AgentChange, AgentDraftAction};
    use finsight_core::{db::run_migrations, keychain, Db};
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("act.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed(conn: &mut Connection) -> (String, String) {
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g','Core',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('dining','g','Dining','#f00',0)", []).unwrap();
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('a','Me','Bank','Checking','Chk','USD','#fff',datetime('now'))", []).unwrap();
        // one uncategorized, one already categorized
        conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES('t_uncat','a','2026-03-01T00:00:00Z',-2500,'Cafe','cleared',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,created_at) VALUES('t_done','a','2026-03-02T00:00:00Z',-3000,'Diner','dining','cleared',datetime('now'))", []).unwrap();
        ("t_uncat".to_string(), "t_done".to_string())
    }

    fn run_tool(conn: &mut Connection, args: Value) -> (Value, Vec<AgentDraftAction>) {
        let mut changes: Vec<AgentChange> = Vec::new();
        let mut drafts: Vec<AgentDraftAction> = Vec::new();
        let out = {
            let mut ctx = ToolContext {
                conn,
                changes: &mut changes,
                draft_actions: &mut drafts,
            };
            draft_recategorization().execute(&mut ctx, args).unwrap()
        };
        (out, drafts)
    }

    #[test]
    fn draft_recategorization_validates_and_drops_invalid_assignments() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        let (t_uncat, t_done) = seed(&mut conn);

        let (out, drafts) = run_tool(
            &mut conn,
            json!({"assignments": [
                {"transaction_id": t_uncat, "category_id": "dining", "confidence": 0.9},
                {"transaction_id": t_done, "category_id": "dining"},        // already categorized -> drop
                {"transaction_id": "ghost", "category_id": "dining"},        // missing txn -> drop
                {"transaction_id": t_uncat, "category_id": "no-such-cat"},   // duplicate + bad cat -> drop
            ]}),
        );

        assert_eq!(out["drafted"], true);
        assert_eq!(out["proposed"], 1, "only the still-uncategorized valid row");
        assert_eq!(out["dropped"], 3);
        assert_eq!(out["requires_approval"], true);

        // Exactly one bulk draft action, nothing written to the DB yet.
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].action_kind, "recategorize_bulk");
        let still_uncat: Option<String> = conn
            .query_row("SELECT category_id FROM transactions WHERE id = 't_uncat'", [], |r| r.get(0))
            .unwrap();
        assert!(still_uncat.is_none(), "draft must not write data");
    }

    #[test]
    fn draft_recategorization_returns_not_drafted_when_all_invalid() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let (out, drafts) = run_tool(
            &mut conn,
            json!({"assignments": [{"transaction_id": "ghost", "category_id": "dining"}]}),
        );
        assert_eq!(out["drafted"], false);
        assert_eq!(out["proposed"], 0);
        assert!(drafts.is_empty());
    }
}
