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
