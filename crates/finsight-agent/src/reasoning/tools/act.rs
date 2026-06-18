use crate::reasoning::messages::AgentChange;
use crate::reasoning::tools::{Tool, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn update_goal_monthly() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "update_goal_monthly" }
        fn description(&self) -> &str { "Update a goal's monthly contribution" }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"goal_id": {"type": "string"}, "new_monthly_cents": {"type": "integer"}}, "required": ["goal_id", "new_monthly_cents"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let goal_id = args["goal_id"].as_str().ok_or_else(|| anyhow::anyhow!("goal_id required"))?;
            let new_monthly = args["new_monthly_cents"].as_i64().ok_or_else(|| anyhow::anyhow!("new_monthly_cents required"))?;

            let old_monthly: i64 = ctx.conn.query_row(
                "SELECT monthly_cents FROM goals WHERE id = ?1", rusqlite::params![goal_id], |r| r.get(0)
            )?;

            ctx.conn.execute(
                "UPDATE goals SET monthly_cents = ?1 WHERE id = ?2", rusqlite::params![new_monthly, goal_id]
            )?;

            let goal_name: String = ctx.conn.query_row(
                "SELECT name FROM goals WHERE id = ?1", rusqlite::params![goal_id], |r| r.get(0)
            )?;

            ctx.changes.push(AgentChange {
                kind: "goal".to_string(),
                description: format!("Updated '{}' goal to ${}/mo (was ${}/mo)", goal_name, new_monthly / 100, old_monthly / 100),
            });

            Ok(json!({"success": true, "goal_id": goal_id, "old_monthly_cents": old_monthly, "new_monthly_cents": new_monthly}))
        }
    }
    Arc::new(T)
}

pub fn create_planned_transaction() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "create_planned_transaction" }
        fn description(&self) -> &str { "Record a future payment, transfer, or investment" }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {
                "description": {"type": "string"},
                "amount_cents": {"type": "integer"},
                "due_date": {"type": "string"},
                "account_id": {"type": "string"},
                "category_id": {"type": "string"}
            }, "required": ["description", "amount_cents", "due_date"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let description = args["description"].as_str().ok_or_else(|| anyhow::anyhow!("description required"))?.to_string();
            let amount = args["amount_cents"].as_i64().ok_or_else(|| anyhow::anyhow!("amount_cents required"))?;
            let due_date = args["due_date"].as_str().ok_or_else(|| anyhow::anyhow!("due_date required"))?.to_string();
            let account_id = args["account_id"].as_str().map(|s| s.to_string());
            let category_id = args["category_id"].as_str().map(|s| s.to_string());

            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            ctx.conn.execute(
                "INSERT INTO planned_transactions (id, description, amount_cents, account_id, category_id, due_date, status, source, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'planned', 'agent', ?7)",
                rusqlite::params![id, description, amount, account_id, category_id, due_date, now],
            )?;

            ctx.changes.push(AgentChange {
                kind: "planned_transaction".to_string(),
                description: format!("Planned '${:.2}' for '{}' on {}", amount as f64 / 100.0, description, due_date),
            });

            Ok(json!({"success": true, "planned_transaction_id": id, "description": description, "amount_cents": amount, "due_date": due_date}))
        }
    }
    Arc::new(T)
}
