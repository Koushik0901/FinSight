# Copilot Deep Reasoning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade FinSight Copilot so it can reason across multiple data sources, call tools, run what-if projections, and autonomously apply safe planning changes (goals and planned transactions) when the user asks complex financial questions.

**Architecture:** Provider-native tool calling (Approach B) — each provider (OpenAI-compat, Anthropic, Ollama) implements tool calling using its native API. A `ReasoningEngine` loops: call LLM → execute tools → feed observations → repeat until final answer. Autonomous actions (update goal monthly, create planned transaction) are executed directly without approval.

**Tech Stack:** Rust/Tauri 2, React 18 + TypeScript + Vite, SQLite/SQLCipher via rusqlite, tanstack-query hooks, sonner toasts, design tokens in `ui/src/styles/tokens.css`

## Global Constraints

- Migration files: `V018__planned_transactions.sql` in `crates/finsight-core/migrations/`
- CSS: use `var(--ink)`, `var(--ink-mute)`, `var(--ink-faint)`, `var(--line)`, `var(--elevated)`, `var(--accent)`, `var(--positive)`, `var(--negative)` — never hardcoded colors
- Icons: import from `ui/src/components/Icons.tsx` using `icon()` factory pattern
- Toasts: `import { toast } from "sonner"` → `toast.success()`, `toast.error()`
- All Rust commands: `#[tauri::command]` + `#[specta::specta]` + `pub async fn`
- After any Rust command change: `cargo run -p finsight-tauri --bin export_bindings`
- Tests: `cd ui && npx vitest run`, `cargo test --workspace`, `cd ui && npx tsc --noEmit`

---

### Task 1: Migration and core model/repo for planned transactions

**Files:**
- Create: `crates/finsight-core/migrations/V018__planned_transactions.sql`
- Create: `crates/finsight-core/src/models/planned_transaction.rs`
- Create: `crates/finsight-core/src/repos/planned_transactions.rs`
- Modify: `crates/finsight-core/src/models/mod.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

**Interfaces:**
- Produces: `PlannedTransaction`, `NewPlannedTransaction`, `PlannedTxnFilter` types
- Produces: `list`, `get`, `insert`, `update_status`, `delete` repo functions

- [ ] **Step 1: Create the migration file**

Create `crates/finsight-core/migrations/V018__planned_transactions.sql`:

```sql
CREATE TABLE IF NOT EXISTS planned_transactions (
  id           TEXT PRIMARY KEY,
  description  TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  account_id   TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  category_id  TEXT REFERENCES categories(id) ON DELETE SET NULL,
  due_date     TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'planned',
  source       TEXT NOT NULL DEFAULT 'agent',
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_planned_txn_due ON planned_transactions(due_date);
CREATE INDEX IF NOT EXISTS idx_planned_txn_status ON planned_transactions(status);
```

- [ ] **Step 2: Create the model file**

Create `crates/finsight-core/src/models/planned_transaction.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTransaction {
    pub id: String,
    pub description: String,
    pub amount_cents: i64,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub due_date: String,
    pub status: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPlannedTransaction {
    pub description: String,
    pub amount_cents: i64,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub due_date: String,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct PlannedTxnFilter {
    pub status: Option<String>,
    pub due_before: Option<String>,
}
```

- [ ] **Step 3: Create the repo file**

Create `crates/finsight-core/src/repos/planned_transactions.rs`:

```rust
use crate::error::CoreResult;
use crate::models::planned_transaction::{NewPlannedTransaction, PlannedTransaction, PlannedTxnFilter};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list(conn: &mut Connection, filter: PlannedTxnFilter) -> CoreResult<Vec<PlannedTransaction>> {
    let mut sql = String::from(
        "SELECT id, description, amount_cents, account_id, category_id, due_date, status, source, created_at \
         FROM planned_transactions"
    );
    let mut conditions: Vec<String> = Vec::new();

    if let Some(ref status) = filter.status {
        conditions.push("status = ?1".to_string());
    }
    if let Some(ref due_before) = filter.due_before {
        conditions.push("due_date <= ?2".to_string());
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY due_date ASC");

    let mut stmt = conn.prepare(&sql)?;

    let rows = if let Some(ref status) = filter.status {
        if let Some(ref due_before) = filter.due_before {
            stmt.query_map(params![status, due_before], map_row)?
        } else {
            stmt.query_map(params![status], map_row)?
        }
    } else if let Some(ref due_before) = filter.due_before {
        stmt.query_map(params![due_before], map_row)?
    } else {
        stmt.query_map([], map_row)?
    };

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<PlannedTransaction> {
    Ok(PlannedTransaction {
        id: r.get(0)?,
        description: r.get(1)?,
        amount_cents: r.get(2)?,
        account_id: r.get(3)?,
        category_id: r.get(4)?,
        due_date: r.get(5)?,
        status: r.get(6)?,
        source: r.get(7)?,
        created_at: r.get(8)?,
    })
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<PlannedTransaction>> {
    let mut stmt = conn.prepare(
        "SELECT id, description, amount_cents, account_id, category_id, due_date, status, source, created_at \
         FROM planned_transactions WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map(params![id], map_row)?;
    Ok(rows.next().transpose()?)
}

pub fn insert(conn: &mut Connection, new: NewPlannedTransaction) -> CoreResult<PlannedTransaction> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO planned_transactions (id, description, amount_cents, account_id, category_id, due_date, status, source, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'planned', ?7, ?8)",
        params![id, new.description, new.amount_cents, new.account_id, new.category_id, new.due_date, new.source, now],
    )?;
    Ok(PlannedTransaction {
        id,
        description: new.description,
        amount_cents: new.amount_cents,
        account_id: new.account_id,
        category_id: new.category_id,
        due_date: new.due_date,
        status: "planned".to_string(),
        source: new.source,
        created_at: now,
    })
}

pub fn update_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE planned_transactions SET status = ?1 WHERE id = ?2",
        params![status, id],
    )?;
    Ok(())
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM planned_transactions WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("planned.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_and_list_planned_transactions() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let planned = insert(
            &mut conn,
            NewPlannedTransaction {
                description: "Pay credit card".to_string(),
                amount_cents: 80000,
                account_id: None,
                category_id: None,
                due_date: "2026-06-25".to_string(),
                source: "agent".to_string(),
            },
        )
        .unwrap();

        assert_eq!(planned.status, "planned");
        assert_eq!(planned.amount_cents, 80000);

        let list = list(&mut conn, PlannedTxnFilter::default()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, planned.id);
    }

    #[test]
    fn update_status_and_delete() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let planned = insert(
            &mut conn,
            NewPlannedTransaction {
                description: "Invest".to_string(),
                amount_cents: 50000,
                account_id: None,
                category_id: None,
                due_date: "2026-06-20".to_string(),
                source: "agent".to_string(),
            },
        )
        .unwrap();

        update_status(&mut conn, &planned.id, "completed").unwrap();
        let fetched = get(&mut conn, &planned.id).unwrap().unwrap();
        assert_eq!(fetched.status, "completed");

        delete(&mut conn, &planned.id).unwrap();
        assert!(get(&mut conn, &planned.id).unwrap().is_none());
    }
}
```

- [ ] **Step 4: Register in models/mod.rs**

Add to `crates/finsight-core/src/models/mod.rs`:

```rust
pub mod planned_transaction;
pub use planned_transaction::{NewPlannedTransaction, PlannedTransaction, PlannedTxnFilter};
```

- [ ] **Step 5: Register in repos/mod.rs**

Add to `crates/finsight-core/src/repos/mod.rs`:

```rust
pub mod planned_transactions;
```

- [ ] **Step 6: Run tests to verify**

Run: `cargo test -p finsight-core --lib repos::planned_transactions::tests`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/finsight-core/migrations/V018__planned_transactions.sql crates/finsight-core/src/models/planned_transaction.rs crates/finsight-core/src/repos/planned_transactions.rs crates/finsight-core/src/models/mod.rs crates/finsight-core/src/repos/mod.rs
git commit -m "feat: add planned transactions migration, model, and repo"
```

---

### Task 2: Extend CompletionProvider trait and add reasoning messages

**Files:**
- Create: `crates/finsight-agent/src/reasoning/mod.rs`
- Create: `crates/finsight-agent/src/reasoning/messages.rs`
- Modify: `crates/finsight-agent/src/lib.rs`
- Modify: `crates/finsight-agent/src/providers/mock.rs`

**Interfaces:**
- Produces: `ChatMessage`, `ToolCall`, `ToolDefinition`, `AssistantTurn`, `ReasoningResult`, `AgentChange` types
- Produces: Extended `CompletionProvider` trait with `complete_tool_turn`

- [ ] **Step 1: Create reasoning module**

Create `crates/finsight-agent/src/reasoning/mod.rs`:

```rust
pub mod messages;
pub mod engine;
pub mod tools;
```

Create `crates/finsight-agent/src/reasoning/messages.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatMessage {
    System { content: String },
    User { content: String },
    Assistant { content: Option<String>, tool_calls: Vec<ToolCall> },
    Tool { tool_call_id: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub enum AssistantTurn {
    ToolCalls(Vec<ToolCall>),
    FinalAnswer { content: String, reasoning: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChange {
    pub kind: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ReasoningResult {
    pub content: String,
    pub reasoning: String,
    pub trace: Vec<String>,
    pub changes: Vec<AgentChange>,
}
```

- [ ] **Step 2: Extend CompletionProvider trait**

Modify `crates/finsight-agent/src/lib.rs`:

```rust
pub mod reasoning;

pub use reasoning::messages::{AgentChange, AssistantTurn, ChatMessage, ReasoningResult, ToolCall, ToolDefinition};
pub use reasoning::tools::{Tool, ToolContext, ToolSet};
pub use reasoning::engine::ReasoningEngine;

use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn model_id(&self) -> &str;

    async fn complete_json(&self, system: &str, user: &str) -> anyhow::Result<Value> {
        Ok(Value::Null)
    }

    async fn complete_tool_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<AssistantTurn> {
        Err(anyhow::anyhow!("Tool calling not implemented for this provider"))
    }
}
```

- [ ] **Step 3: Update MockCompletionProvider for tool turns**

Modify `crates/finsight-agent/src/providers/mock.rs`:

```rust
use crate::CompletionProvider;
use crate::reasoning::messages::{AssistantTurn, ChatMessage, ToolDefinition};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Mutex;
use serde_json::Value;

pub struct MockCompletionProvider {
    pub provider_id: String,
    pub model_id: String,
    pub response: Value,
    pub tool_turns: Mutex<Vec<AssistantTurn>>,
}

#[async_trait]
impl CompletionProvider for MockCompletionProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }
    fn model_id(&self) -> &str {
        &self.model_id
    }
    async fn complete_json(&self, _system: &str, _user: &str) -> Result<Value> {
        Ok(self.response.clone())
    }
    async fn complete_tool_turn(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        let mut turns = self.tool_turns.lock().unwrap();
        if turns.is_empty() {
            Ok(AssistantTurn::FinalAnswer {
                content: "No more turns scripted".to_string(),
                reasoning: "Test exhausted".to_string(),
            })
        } else {
            Ok(turns.remove(0))
        }
    }
}
```

- [ ] **Step 4: Create tools/mod.rs and engine.rs stubs**

Create `crates/finsight-agent/src/reasoning/tools/mod.rs`:

```rust
pub mod read;
pub mod act;

use crate::reasoning::messages::ToolDefinition;
use anyhow::Result;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value>;
}

pub struct ToolContext<'a> {
    pub conn: &'a mut Connection,
    pub changes: &'a mut Vec<crate::reasoning::messages::AgentChange>,
}

pub struct ToolSet {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolSet {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| ToolDefinition {
            name: t.name().to_string(),
            description: t.description().to_string(),
            parameters: t.parameters(),
        }).collect()
    }
    pub fn execute(&self, name: &str, ctx: &mut ToolContext, args: Value) -> Result<Value> {
        let tool = self.tools.get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", name))?;
        tool.execute(ctx, args)
    }
}
```

Create `crates/finsight-agent/src/reasoning/engine.rs`:

```rust
use crate::reasoning::messages::{AgentChange, AssistantTurn, ChatMessage, ReasoningResult, ToolCall};
use crate::reasoning::tools::{ToolContext, ToolSet};
use crate::CompletionProvider;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

pub struct ReasoningEngine;

impl ReasoningEngine {
    pub async fn run(
        conn: &mut rusqlite::Connection,
        question: &str,
        tools: &ToolSet,
        provider: Arc<dyn CompletionProvider>,
        max_iterations: usize,
    ) -> Result<ReasoningResult> {
        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage::System { content: Self::build_system_prompt(tools) },
            ChatMessage::User { content: question.to_string() },
        ];
        let mut trace: Vec<String> = Vec::new();
        let mut changes: Vec<AgentChange> = Vec::new();

        for _ in 0..max_iterations {
            let turn = provider.complete_tool_turn(&messages, &tools.definitions()).await?;

            match turn {
                AssistantTurn::ToolCalls(calls) => {
                    let mut tool_result_msgs = Vec::new();
                    for call in &calls {
                        trace.push(format!("Called tool: {}", call.name));
                        let mut ctx = ToolContext { conn, changes: &mut changes };
                        let result = tools.execute(&call.name, &mut ctx, call.arguments.clone())?;
                        tool_result_msgs.push(ChatMessage::Tool {
                            tool_call_id: call.id.clone(),
                            content: result.to_string(),
                        });
                    }
                    messages.push(ChatMessage::Assistant {
                        content: None,
                        tool_calls: calls,
                    });
                    for msg in tool_result_msgs {
                        messages.push(msg);
                    }
                }
                AssistantTurn::FinalAnswer { content, reasoning } => {
                    return Ok(ReasoningResult {
                        content,
                        reasoning,
                        trace,
                        changes,
                    });
                }
            }
        }

        Ok(ReasoningResult {
            content: "I analyzed your finances but ran out of reasoning steps. Here's what I found so far.".to_string(),
            reasoning: "The question was too complex for the iteration limit.".to_string(),
            trace,
            changes,
        })
    }

    fn build_system_prompt(tools: &ToolSet) -> String {
        let tool_defs = tools.definitions();
        let tool_list: String = tool_defs.iter().map(|t| {
            format!("- {}: {} Parameters: {}", t.name, t.description, t.parameters)
        }).collect::<Vec<_>>().join("\n");

        format!(
            "You are a personal financial analyst for a local-first personal finance app.\n\
             You have access to the following tools:\n{}\n\n\
             When you need data, call the appropriate tool(s).\n\
             When you have enough information to answer, provide your final answer with reasoning.\n\
             Be specific with numbers. Explain your reasoning clearly.\n\
             Autonomous actions (update_goal_monthly, create_planned_transaction) are allowed.\n\
             Respond with either tool calls or a final answer.", tool_list
        )
    }
}
```

- [ ] **Step 5: Run tests to verify compilation**

Run: `cargo test -p finsight-agent --lib`
Expected: Tests compile and pass.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-agent/src/reasoning/ crates/finsight-agent/src/lib.rs crates/finsight-agent/src/providers/mock.rs
git commit -m "feat: add reasoning engine skeleton with messages and tool registry"
```

---

### Task 3: Implement read-only tools

**Files:**
- Modify: `crates/finsight-agent/src/reasoning/tools/read.rs`
- Create: `crates/finsight-agent/src/reasoning/tools/mod.rs` (already exists)

**Interfaces:**
- Consumes: `ToolContext` from Task 2
- Produces: 9 read-only tool implementations

- [ ] **Step 1: Create read-only tools**

Create `crates/finsight-agent/src/reasoning/tools/read.rs`:

```rust
use crate::reasoning::tools::{Tool, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn get_account_balances() -> Arc<dyn Tool> {
    struct T;
    #[async_trait::async_trait]
    impl Tool for T {
        fn name(&self) -> &str { "get_account_balances" }
        fn description(&self) -> &str { "Get current balance for every account plus total" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let mut stmt = ctx.conn.prepare(
                "SELECT a.name, COALESCE((SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY as_of_date DESC LIMIT 1), 0) AS balance \
                 FROM accounts a WHERE a.archived_at IS NULL ORDER BY a.name"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                Ok(json!({"name": r.get::<_, String>(0)?, "balance_cents": r.get::<_, i64>(1)?}))
            })?.filter_map(|r| r.ok()).collect();
            let total: i64 = rows.iter().filter_map(|r| r["balance_cents"].as_i64()).sum();
            Ok(json!({"accounts": rows, "total_cents": total}))
        }
    }
    Arc::new(T)
}

pub fn get_month_totals() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_month_totals" }
        fn description(&self) -> &str { "Get this month's income, expenses, and savings rate" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let now = chrono::Utc::now();
            let month_start = now.format("%Y-%m-01").to_string();
            let (income, expense): (i64, i64) = ctx.conn.query_row(
                "SELECT COALESCE(SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END),0), \
                        COALESCE(SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END),0) \
                 FROM transactions WHERE posted_at >= ?1",
                rusqlite::params![month_start],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let savings_rate = if income > 0 { ((income - expense) * 100 / income).max(0) } else { 0 };
            Ok(json!({"income_cents": income, "expense_cents": expense, "net_cents": income - expense, "savings_rate_pct": savings_rate}))
        }
    }
    Arc::new(T)
}

pub fn get_top_spending_categories() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_top_spending_categories" }
        fn description(&self) -> &str { "Get top spending categories with amounts" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {"limit": {"type": "integer", "default": 5}}}) }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let limit = args["limit"].as_i64().unwrap_or(5);
            let now = chrono::Utc::now();
            let month_start = now.format("%Y-%m-01").to_string();
            let mut stmt = ctx.conn.prepare(
                "SELECT c.label, SUM(ABS(t.amount_cents)) AS spent \
                 FROM transactions t JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.posted_at >= ?1 \
                 GROUP BY c.id ORDER BY spent DESC LIMIT ?2"
            )?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params![month_start, limit], |r| {
                Ok(json!({"category": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"categories": rows}))
        }
    }
    Arc::new(T)
}

pub fn get_budgets() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_budgets" }
        fn description(&self) -> &str { "Get current month budgets with budgeted vs actual" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let now = chrono::Utc::now();
            let month = now.format("%Y-%m").to_string();
            let month_start = now.format("%Y-%m-01").to_string();
            let mut stmt = ctx.conn.prepare(
                "SELECT c.label, b.amount_cents, \
                        COALESCE((SELECT SUM(ABS(t.amount_cents)) FROM transactions t \
                                  WHERE t.category_id = b.category_id AND t.amount_cents < 0 AND t.posted_at >= ?1), 0) AS spent \
                 FROM budgets b JOIN categories c ON c.id = b.category_id WHERE b.month = ?2"
            )?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params![month_start, month], |r| {
                Ok(json!({"category": r.get::<_, String>(0)?, "budget_cents": r.get::<_, i64>(1)?, "spent_cents": r.get::<_, i64>(2)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"budgets": rows, "month": month}))
        }
    }
    Arc::new(T)
}

pub fn get_goals() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_goals" }
        fn description(&self) -> &str { "Get goals with current balance, target, monthly contribution" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let mut stmt = ctx.conn.prepare(
                "SELECT name, target_cents, current_cents, monthly_cents FROM goals WHERE archived_at IS NULL ORDER BY sort_order"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                let target: i64 = r.get(1)?;
                let current: i64 = r.get(2)?;
                let pct = if target > 0 { current * 100 / target } else { 0 };
                Ok(json!({"name": r.get::<_, String>(0)?, "target_cents": target, "current_cents": current, "monthly_cents": r.get::<_, i64>(3)?, "progress_pct": pct}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"goals": rows}))
        }
    }
    Arc::new(T)
}

pub fn get_recurring_bills() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_recurring_bills" }
        fn description(&self) -> &str { "Get detected recurring bills with expected next date" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {"days_ahead": {"type": "integer", "default": 30}}}) }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let _days = args["days_ahead"].as_i64().unwrap_or(30);
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(395)).format("%Y-%m-%d").to_string();
            let mut stmt = ctx.conn.prepare(
                "WITH gaps AS ( \
                    SELECT merchant_raw, date(posted_at) AS d, LAG(date(posted_at)) OVER (PARTITION BY merchant_raw ORDER BY posted_at) AS prev_d \
                    FROM transactions WHERE posted_at >= ?1 \
                 ), agg AS ( \
                    SELECT merchant_raw, AVG(julianday(d)-julianday(prev_d)) AS avg_gap, MAX(d) AS last_seen, MAX(amount_cents) AS last_amount, COUNT(*) AS occ \
                    FROM gaps WHERE prev_d IS NOT NULL GROUP BY merchant_raw HAVING occ >= 2 AND AVG(julianday(d)-julianday(prev_d)) BETWEEN 5 AND 400 \
                 ) SELECT merchant_raw, avg_gap, last_seen, last_amount FROM agg ORDER BY ABS(last_amount) DESC"
            )?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params![cutoff], |r| {
                let avg_gap: f64 = r.get(1)?;
                let last_seen: String = r.get(2)?;
                let next = chrono::NaiveDate::parse_from_str(&last_seen, "%Y-%m-%d")
                    .map(|d| (d + chrono::Duration::days(avg_gap.round() as i64)).format("%Y-%m-%d").to_string())
                    .unwrap_or(last_seen.clone());
                Ok(json!({"merchant": r.get::<_, String>(0)?, "avg_gap_days": avg_gap, "last_seen": last_seen, "next_expected": next, "last_amount_cents": r.get::<_, i64>(3)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"recurring_bills": rows}))
        }
    }
    Arc::new(T)
}

pub fn get_liabilities() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_liabilities" }
        fn description(&self) -> &str { "Get credit cards and loans with balance, APR, minimum payment" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let mut stmt = ctx.conn.prepare(
                "SELECT name, balance_cents, apr_pct, limit_cents FROM liabilities WHERE archived_at IS NULL ORDER BY balance_cents DESC"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                Ok(json!({"name": r.get::<_, String>(0)?, "balance_cents": r.get::<_, i64>(1)?, "apr_pct": r.get::<_, f64>(2)?, "limit_cents": r.get::<_, Option<i64>>(3)?}))
            })?.filter_map(|r| r.ok()).collect();
            let total: i64 = rows.iter().filter_map(|r| r["balance_cents"].as_i64()).sum();
            Ok(json!({"liabilities": rows, "total_cents": total}))
        }
    }
    Arc::new(T)
}

pub fn search_transactions() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "search_transactions" }
        fn description(&self) -> &str { "Find transactions by merchant, date range, category, or amount" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {"merchant": {"type": "string"}, "start_date": {"type": "string"}, "end_date": {"type": "string"}, "limit": {"type": "integer", "default": 10}}}) }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let mut sql = "SELECT t.merchant_raw, t.amount_cents, t.posted_at, COALESCE(c.label, 'Uncategorized') FROM transactions t LEFT JOIN categories c ON c.id = t.category_id WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            if let Some(m) = args["merchant"].as_str() {
                sql.push_str(" AND lower(t.merchant_raw) LIKE lower(?)");
                params.push(Box::new(format!("%{}%", m)));
            }
            if let Some(s) = args["start_date"].as_str() {
                sql.push_str(" AND t.posted_at >= ?");
                params.push(Box::new(s.to_string()));
            }
            if let Some(e) = args["end_date"].as_str() {
                sql.push_str(" AND t.posted_at <= ?");
                params.push(Box::new(format!("{}T23:59:59", e)));
            }
            let limit = args["limit"].as_i64().unwrap_or(10);
            sql.push_str(" ORDER BY t.posted_at DESC LIMIT ?");
            params.push(Box::new(limit));

            let mut stmt = ctx.conn.prepare(&sql)?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())), |r| {
                Ok(json!({"merchant": r.get::<_, String>(0)?, "amount_cents": r.get::<_, i64>(1)?, "date": r.get::<_, String>(2)?, "category": r.get::<_, String>(3)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"transactions": rows, "count": rows.len()}))
        }
    }
    Arc::new(T)
}

pub fn run_cashflow_projection() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "run_cashflow_projection" }
        fn description(&self) -> &str { "Project runway and end-of-month net under hypothetical changes" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {"months": {"type": "integer", "default": 3}, "extra_monthly_expense_cents": {"type": "integer", "default": 0}, "extra_monthly_income_cents": {"type": "integer", "default": 0}}}) }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let months = args["months"].as_i64().unwrap_or(3);
            let extra_expense = args["extra_monthly_expense_cents"].as_i64().unwrap_or(0);
            let extra_income = args["extra_monthly_income_cents"].as_i64().unwrap_or(0);
            let now = chrono::Utc::now();
            let month_start = now.format("%Y-%m-01").to_string();
            let day_of_month = now.format("%d").to_string().parse::<i64>().unwrap_or(15);
            let (income, expense): (i64, i64) = ctx.conn.query_row(
                "SELECT COALESCE(SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END),0), \
                        COALESCE(SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END),0) \
                 FROM transactions WHERE posted_at >= ?1",
                rusqlite::params![month_start],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let balance: i64 = ctx.conn.query_row(
                "SELECT COALESCE(SUM(balance_cents), 0) FROM accounts WHERE archived_at IS NULL", [], |r| r.get(0)
            )?;
            let daily_net = if day_of_month > 0 { (income - expense) / day_of_month } else { 0 };
            let avg_daily_burn = if day_of_month > 0 { expense / day_of_month } else { 0 };
            let runway_days = if avg_daily_burn > 0 { balance / avg_daily_burn } else { 9999 };
            let projected_monthly_net = (income + extra_income) - (expense + extra_expense);
            let projections: Vec<Value> = (1..=months).map(|m| {
                json!({"month": m, "projected_net_cents": projected_monthly_net * m, "projected_balance_cents": balance + projected_monthly_net * m})
            }).collect();
            Ok(json!({"current_balance_cents": balance, "monthly_income_cents": income, "monthly_expense_cents": expense, "daily_net_cents": daily_net, "runway_days": runway_days, "projections": projections}))
        }
    }
    Arc::new(T)
}
```

- [ ] **Step 2: Create empty act.rs**

Create `crates/finsight-agent/src/reasoning/tools/act.rs`:

```rust
// Action tools will be implemented in Task 4
```

- [ ] **Step 3: Run tests to verify compilation**

Run: `cargo test -p finsight-agent --lib`
Expected: Tests compile and pass.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-agent/src/reasoning/tools/
git commit -m "feat: implement read-only tools for reasoning engine"
```

---

### Task 4: Implement action tools

**Files:**
- Modify: `crates/finsight-agent/src/reasoning/tools/act.rs`

**Interfaces:**
- Consumes: `ToolContext` from Task 2
- Produces: 2 action tool implementations

- [ ] **Step 1: Create action tools**

Replace `crates/finsight-agent/src/reasoning/tools/act.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify compilation**

Run: `cargo test -p finsight-agent --lib`
Expected: Tests compile and pass.

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-agent/src/reasoning/tools/act.rs
git commit -m "feat: implement action tools for reasoning engine"
```

---

### Task 5: Implement provider-native tool calling for OpenAI-compat

**Files:**
- Modify: `crates/finsight-agent/src/providers/openai_compat.rs`

**Interfaces:**
- Consumes: `ChatMessage`, `ToolDefinition`, `AssistantTurn` from Task 2
- Produces: `complete_tool_turn` implementation

- [ ] **Step 1: Implement tool calling for OpenAI-compat provider**

Add to `crates/finsight-agent/src/providers/openai_compat.rs`:

```rust
use crate::reasoning::messages::{AssistantTurn, ChatMessage, ToolCall, ToolDefinition};

#[derive(Deserialize)]
struct OaiToolCall {
    id: String,
    function: OaiFunction,
}

#[derive(Deserialize)]
struct OaiFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OaiMessageWithTools {
    content: Option<String>,
    tool_calls: Option<Vec<OaiToolCall>>,
}

#[derive(Deserialize)]
struct OaiChoiceWithTools {
    message: OaiMessageWithTools,
}

#[derive(Deserialize)]
struct OaiRespWithTools {
    choices: Vec<OaiChoiceWithTools>,
}
```

Add to the `impl CompletionProvider for OpenAiCompatProvider` block:

```rust
async fn complete_tool_turn(
    &self,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<AssistantTurn> {
    let oai_messages: Vec<Value> = messages.iter().map(|m| {
        match m {
            ChatMessage::System { content } => json!({"role": "system", "content": content}),
            ChatMessage::User { content } => json!({"role": "user", "content": content}),
            ChatMessage::Assistant { content, tool_calls } => {
                let mut msg = json!({"role": "assistant"});
                if let Some(c) = content { msg["content"] = json!(c); }
                if !tool_calls.is_empty() {
                    msg["tool_calls"] = json!(tool_calls.iter().map(|tc| {
                        json!({"id": tc.id, "type": "function", "function": {"name": tc.name, "arguments": tc.arguments.to_string()}})
                    }).collect::<Vec<_>>());
                }
                msg
            }
            ChatMessage::Tool { tool_call_id, content } => {
                json!({"role": "tool", "tool_call_id": tool_call_id, "content": content})
            }
        }
    }).collect();

    let oai_tools: Vec<Value> = tools.iter().map(|t| {
        json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.parameters}})
    }).collect();

    let body = json!({
        "model": self.model,
        "messages": oai_messages,
        "tools": oai_tools,
    });

    let resp: OaiRespWithTools = self.client
        .post(format!("{}/chat/completions", self.base_url))
        .bearer_auth(&self.api_key)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let choice = resp.choices.into_iter().next().ok_or_else(|| anyhow!("no choices"))?;
    let msg = choice.message;

    if let Some(tool_calls) = msg.tool_calls {
        if !tool_calls.is_empty() {
            let calls: Vec<ToolCall> = tool_calls.into_iter().map(|tc| {
                let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                ToolCall { id: tc.id, name: tc.function.name, arguments: args }
            }).collect();
            return Ok(AssistantTurn::ToolCalls(calls));
        }
    }

    let content = msg.content.unwrap_or_default();
    Ok(AssistantTurn::FinalAnswer { content, reasoning: "".to_string() })
}
```

- [ ] **Step 2: Run tests to verify compilation**

Run: `cargo test -p finsight-agent --lib`
Expected: Tests compile and pass.

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-agent/src/providers/openai_compat.rs
git commit -m "feat: implement tool calling for OpenAI-compat provider"
```

---

### Task 6: Implement provider-native tool calling for Anthropic

**Files:**
- Modify: `crates/finsight-agent/src/providers/anthropic.rs`

**Interfaces:**
- Consumes: `ChatMessage`, `ToolDefinition`, `AssistantTurn` from Task 2
- Produces: `complete_tool_turn` implementation

- [ ] **Step 1: Implement tool calling for Anthropic provider**

Add to `crates/finsight-agent/src/providers/anthropic.rs`:

```rust
use crate::reasoning::messages::{AssistantTurn, ChatMessage, ToolCall, ToolDefinition};

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<Value>,
}

#[derive(Deserialize)]
struct AnthropicRespWithTools {
    content: Vec<AnthropicContentBlock>,
}
```

Add to the `impl CompletionProvider for AnthropicProvider` block:

```rust
async fn complete_tool_turn(
    &self,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<AssistantTurn> {
    let mut system_msg = String::new();
    let mut api_messages: Vec<Value> = Vec::new();

    for m in messages {
        match m {
            ChatMessage::System { content } => system_msg = content.clone(),
            ChatMessage::User { content } => api_messages.push(json!({"role": "user", "content": content})),
            ChatMessage::Assistant { content, tool_calls } => {
                let mut blocks: Vec<Value> = Vec::new();
                if let Some(c) = content { blocks.push(json!({"type": "text", "text": c})); }
                for tc in tool_calls {
                    blocks.push(json!({"type": "tool_use", "id": tc.id, "name": tc.name, "input": tc.arguments}));
                }
                api_messages.push(json!({"role": "assistant", "content": blocks}));
            }
            ChatMessage::Tool { tool_call_id, content } => {
                api_messages.push(json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": tool_call_id, "content": content}]}));
            }
        }
    }

    let api_tools: Vec<Value> = tools.iter().map(|t| {
        json!({"name": t.name, "description": t.description, "input_schema": t.parameters})
    }).collect();

    let body = json!({
        "model": self.model,
        "max_tokens": 4096,
        "system": system_msg,
        "messages": api_messages,
        "tools": api_tools,
    });

    let resp: AnthropicRespWithTools = self.client
        .post(ANTHROPIC_API)
        .header("x-api-key", &self.api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for block in resp.content {
        match block.kind.as_str() {
            "text" => { if let Some(t) = block.text { text_parts.push(t); } }
            "tool_use" => {
                tool_calls.push(ToolCall {
                    id: block.id.unwrap_or_default(),
                    name: block.name.unwrap_or_default(),
                    arguments: block.input.unwrap_or(json!({})),
                });
            }
            _ => {}
        }
    }

    if !tool_calls.is_empty() {
        Ok(AssistantTurn::ToolCalls(tool_calls))
    } else {
        Ok(AssistantTurn::FinalAnswer {
            content: text_parts.join("\n"),
            reasoning: "".to_string(),
        })
    }
}
```

- [ ] **Step 2: Run tests to verify compilation**

Run: `cargo test -p finsight-agent --lib`
Expected: Tests compile and pass.

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-agent/src/providers/anthropic.rs
git commit -m "feat: implement tool calling for Anthropic provider"
```

---

### Task 7: Implement provider-native tool calling for Ollama

**Files:**
- Modify: `crates/finsight-agent/src/providers/ollama.rs`

**Interfaces:**
- Consumes: `ChatMessage`, `ToolDefinition`, `AssistantTurn` from Task 2
- Produces: `complete_tool_turn` implementation

- [ ] **Step 1: Implement tool calling for Ollama provider**

Add to `crates/finsight-agent/src/providers/ollama.rs`:

```rust
use crate::reasoning::messages::{AssistantTurn, ChatMessage, ToolCall, ToolDefinition};

#[derive(Deserialize)]
struct OllamaToolCall {
    function: OllamaFunction,
}

#[derive(Deserialize)]
struct OllamaFunction {
    name: String,
    arguments: Value,
}

#[derive(Deserialize)]
struct OllamaMessageWithTools {
    content: Option<String>,
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Deserialize)]
struct OllamaRespWithTools {
    message: OllamaMessageWithTools,
}
```

Add to the `impl CompletionProvider for OllamaProvider` block:

```rust
async fn complete_tool_turn(
    &self,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<AssistantTurn> {
    let ollama_messages: Vec<Value> = messages.iter().map(|m| {
        match m {
            ChatMessage::System { content } => json!({"role": "system", "content": content}),
            ChatMessage::User { content } => json!({"role": "user", "content": content}),
            ChatMessage::Assistant { content, tool_calls } => {
                let mut msg = json!({"role": "assistant"});
                if let Some(c) = content { msg["content"] = json!(c); }
                if !tool_calls.is_empty() {
                    msg["tool_calls"] = json!(tool_calls.iter().map(|tc| {
                        json!({"function": {"name": tc.name, "arguments": tc.arguments}})
                    }).collect::<Vec<_>>());
                }
                msg
            }
            ChatMessage::Tool { tool_call_id, content } => {
                json!({"role": "tool", "content": content})
            }
        }
    }).collect();

    let ollama_tools: Vec<Value> = tools.iter().map(|t| {
        json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.parameters}})
    }).collect();

    let body = json!({
        "model": self.model,
        "stream": false,
        "messages": ollama_messages,
        "tools": ollama_tools,
    });

    let resp: OllamaRespWithTools = self.client
        .post(format!("{}/api/chat", self.base_url))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let msg = resp.message;
    if let Some(tool_calls) = msg.tool_calls {
        if !tool_calls.is_empty() {
            let calls: Vec<ToolCall> = tool_calls.into_iter().enumerate().map(|(i, tc)| {
                ToolCall {
                    id: format!("call_{}", i),
                    name: tc.function.name,
                    arguments: tc.function.arguments,
                }
            }).collect();
            return Ok(AssistantTurn::ToolCalls(calls));
        }
    }

    let content = msg.content.unwrap_or_default();
    Ok(AssistantTurn::FinalAnswer { content, reasoning: "".to_string() })
}
```

- [ ] **Step 2: Run tests to verify compilation**

Run: `cargo test -p finsight-agent --lib`
Expected: Tests compile and pass.

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-agent/src/providers/ollama.rs
git commit -m "feat: implement tool calling for Ollama provider"
```

---

### Task 8: Write reasoning engine tests

**Files:**
- Create: `crates/finsight-agent/src/reasoning/engine_test.rs`

**Interfaces:**
- Consumes: `MockCompletionProvider`, `ToolSet`, `ReasoningEngine` from earlier tasks
- Produces: 4 unit tests for the engine

- [ ] **Step 1: Create engine tests**

Create `crates/finsight-agent/src/reasoning/engine_test.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::super::engine::ReasoningEngine;
    use super::super::messages::{AssistantTurn, ToolCall};
    use super::super::tools::{ToolSet, read, act};
    use crate::providers::mock::MockCompletionProvider;
    use finsight_core::{db::run_migrations, keychain, Db};
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("engine.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn build_toolset() -> ToolSet {
        let mut tools = ToolSet::new();
        tools.register(read::get_account_balances());
        tools.register(read::get_month_totals());
        tools.register(read::get_goals());
        tools.register(act::update_goal_monthly());
        tools.register(act::create_planned_transaction());
        tools
    }

    #[tokio::test]
    async fn single_turn_final_answer() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!({}),
            tool_turns: Mutex::new(vec![AssistantTurn::FinalAnswer {
                content: "Your savings rate is 20%".to_string(),
                reasoning: "Based on income and expenses".to_string(),
            }]),
        });
        let tools = build_toolset();
        let result = ReasoningEngine::run(&mut conn, "What is my savings rate?", &tools, provider, 5).await.unwrap();
        assert!(result.content.contains("20%"));
        assert!(result.trace.is_empty());
    }

    #[tokio::test]
    async fn multi_turn_with_tool_calls() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!({}),
            tool_turns: Mutex::new(vec![
                AssistantTurn::ToolCalls(vec![ToolCall {
                    id: "call_1".into(),
                    name: "get_account_balances".into(),
                    arguments: json!({}),
                }]),
                AssistantTurn::FinalAnswer {
                    content: "You have $5000 across all accounts".to_string(),
                    reasoning: "Summed account balances".to_string(),
                },
            ]),
        });
        let tools = build_toolset();
        let result = ReasoningEngine::run(&mut conn, "What are my account balances?", &tools, provider, 5).await.unwrap();
        assert!(result.content.contains("5000"));
        assert_eq!(result.trace.len(), 1);
        assert!(result.trace[0].contains("get_account_balances"));
    }

    #[tokio::test]
    async fn max_iterations_returns_partial() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!({}),
            tool_turns: Mutex::new(vec![
                AssistantTurn::ToolCalls(vec![ToolCall {
                    id: "call_1".into(),
                    name: "get_account_balances".into(),
                    arguments: json!({}),
                }]),
                AssistantTurn::ToolCalls(vec![ToolCall {
                    id: "call_2".into(),
                    name: "get_month_totals".into(),
                    arguments: json!({}),
                }]),
            ]),
        });
        let tools = build_toolset();
        let result = ReasoningEngine::run(&mut conn, "Complex question", &tools, provider, 2).await.unwrap();
        assert!(result.trace.len() <= 2);
        assert!(result.content.contains("ran out of reasoning steps"));
    }

    #[tokio::test]
    async fn action_tool_records_change() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        // Insert a goal first
        conn.execute(
            "INSERT INTO goals (id, name, type, target_cents, current_cents, monthly_cents, color, sort_order, created_at) VALUES ('g1', 'Invest', 'save', 100000, 20000, 10000, '#fff', 0, datetime('now'))",
            [],
        ).unwrap();
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!({}),
            tool_turns: Mutex::new(vec![
                AssistantTurn::ToolCalls(vec![ToolCall {
                    id: "call_1".into(),
                    name: "update_goal_monthly".into(),
                    arguments: json!({"goal_id": "g1", "new_monthly_cents": 25000}),
                }]),
                AssistantTurn::FinalAnswer {
                    content: "Updated your invest goal".to_string(),
                    reasoning: "Increased contribution".to_string(),
                },
            ]),
        });
        let tools = build_toolset();
        let result = ReasoningEngine::run(&mut conn, "Increase my invest goal", &tools, provider, 5).await.unwrap();
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].kind, "goal");
    }
}
```

- [ ] **Step 2: Add test module to engine.rs**

Add at the bottom of `crates/finsight-agent/src/reasoning/engine.rs`:

```rust
#[cfg(test)]
mod tests;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p finsight-agent --lib reasoning::engine::tests`
Expected: All 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-agent/src/reasoning/
git commit -m "test: add reasoning engine unit tests"
```

---

### Task 9: Extend ask_agent command with mode and router

**Files:**
- Modify: `crates/finsight-app/src/commands/agent.rs`
- Modify: `crates/finsight-app/src/lib.rs` (register command)

**Interfaces:**
- Consumes: `ReasoningEngine`, `ToolSet`, `ToolContext` from earlier tasks
- Produces: Extended `ask_agent` with `mode` parameter

- [ ] **Step 1: Extend AgentAnswer and ask_agent command**

Add to `crates/finsight-app/src/commands/agent.rs`:

```rust
use finsight_agent::{reasoning::{messages::AgentChange, engine::ReasoningEngine, tools::{ToolSet, read, act}}, CompletionProvider};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentAnswer {
    pub prose: String,
    pub reasoning: String,
    pub trace: Vec<String>,
    pub changes: Vec<AgentChange>,
    pub action_label: Option<String>,
    pub action_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentChange {
    pub kind: String,
    pub description: String,
}

fn build_toolset() -> ToolSet {
    let mut tools = ToolSet::new();
    tools.register(read::get_account_balances());
    tools.register(read::get_month_totals());
    tools.register(read::get_top_spending_categories());
    tools.register(read::get_budgets());
    tools.register(read::get_goals());
    tools.register(read::get_recurring_bills());
    tools.register(read::get_liabilities());
    tools.register(read::search_transactions());
    tools.register(read::run_cashflow_projection());
    tools.register(act::update_goal_monthly());
    tools.register(act::create_planned_transaction());
    tools
}

async fn router_classify(provider: &Arc<dyn CompletionProvider>, question: &str) -> String {
    let system = "Classify this question as 'simple' (greetings, general info, single-fact lookups) or 'deep' (financial planning, pay allocation, investment decisions, debt payoff, should-I questions). Respond with JSON only: {\"mode\": \"simple\" | \"deep\"}";
    match provider.complete_json(system, question).await {
        Ok(v) => {
            if let Some(mode) = v.get("mode").and_then(|m| m.as_str()) {
                if mode == "deep" { return "deep".to_string(); }
            }
            "simple".to_string()
        }
        Err(_) => "simple".to_string(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn ask_agent(
    state: tauri::State<'_, AppState>,
    question: String,
    mode: Option<String>,
) -> AppResult<AgentAnswer> {
    let provider = state.agent_provider.read().unwrap().clone();
    let Some(provider) = provider else {
        return Err(AppError::new("no_provider", "Configure an AI provider in Settings → Agent to use this feature."));
    };

    let effective_mode = match mode.as_deref() {
        Some("deep") => "deep".to_string(),
        Some("quick") => "simple".to_string(),
        _ => router_classify(&provider, &question).await,
    };

    let db = (*state.db).clone();

    if effective_mode == "deep" {
        let tools = build_toolset();
        let result = run(&db, move |conn| {
            finsight_agent::tokio_runtime().block_on(async {
                ReasoningEngine::run(conn, &question, &tools, provider, 10).await
            })
        }).await.map_err(AppError::from)?;

        // Persist executed bundle
        let changes_for_bundle: Vec<AgentChange> = result.changes.iter().map(|c| AgentChange { kind: c.kind.clone(), description: c.description.clone() }).collect();
        let _ = run(&db, move |mut conn| {
            let bundle = finsight_core::repos::copilot_actions::insert_bundle(
                &mut conn, None, &question, &result.content, &result.reasoning, 1.0, Some(provider.provider_id()), Some(provider.model_id())
            )?;
            for (i, change) in changes_for_bundle.iter().enumerate() {
                finsight_core::repos::copilot_actions::insert_item(
                    &mut conn, &bundle.id, &change.kind, "{}", &change.description, 1.0, i as i64
                )?;
            }
            finsight_core::repos::copilot_actions::set_bundle_status(&mut conn, &bundle.id, "executed")?;
            Ok::<_, finsight_core::CoreError>(())
        }).await;

        Ok(AgentAnswer {
            prose: result.content,
            reasoning: result.reasoning,
            trace: result.trace,
            changes: result.changes.into_iter().map(|c| AgentChange { kind: c.kind, description: c.description }).collect(),
            action_label: None,
            action_path: None,
        })
    } else {
        // Simple path - existing single-shot logic
        let context = run(&db, |conn| {
            // ... existing context building ...
            Ok("Financial context placeholder".to_string())
        }).await.map_err(AppError::from)?;

        let system = format!("You are a personal finance assistant. Answer concisely.\n\nFinancial context:\n{context}");
        let raw = provider.complete_json(&system, &question).await.map_err(|e| AppError::new("ask_agent.llm", e.to_string()))?;
        let prose = raw.get("prose").and_then(|v| v.as_str()).unwrap_or("I couldn't generate a response.").to_string();
        let action_label = raw.get("action_label").and_then(|v| v.as_str()).map(|s| s.to_string());
        let action_path = raw.get("action_path").and_then(|v| v.as_str()).map(|s| s.to_string());

        Ok(AgentAnswer {
            prose,
            reasoning: String::new(),
            trace: Vec::new(),
            changes: Vec::new(),
            action_label,
            action_path,
        })
    }
}
```

- [ ] **Step 2: Update build_specta_builder**

Modify `crates/finsight-app/src/lib.rs` to update the `ask_agent` command registration (ensure it matches the new signature with `mode: Option<String>`).

- [ ] **Step 3: Run tests**

Run: `cargo test -p finsight-app`
Expected: Tests compile and pass.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/lib.rs
git commit -m "feat: extend ask_agent with deep reasoning mode and router"
```

---

### Task 10: Regenerate TypeScript bindings

**Files:**
- Modify: `ui/src/api/bindings.ts` (auto-generated)

**Interfaces:**
- Consumes: Updated Rust commands from Task 9
- Produces: Updated TypeScript bindings

- [ ] **Step 1: Regenerate bindings**

Run from repo root: `cargo run -p finsight-tauri --bin export_bindings`
Expected: `ui/src/api/bindings.ts` updated with new `askAgent` signature and `AgentAnswer` type.

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd ui && npx tsc --noEmit`
Expected: No TypeScript errors.

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/bindings.ts
git commit -m "chore: regenerate TypeScript bindings"
```

---

### Task 11: Update frontend hook for deep mode

**Files:**
- Modify: `ui/src/api/hooks/agent.ts`

**Interfaces:**
- Consumes: Updated bindings from Task 10
- Produces: Updated `useAskAgent` hook

- [ ] **Step 1: Update useAskAgent hook**

Modify `ui/src/api/hooks/agent.ts`:

```typescript
export function useAskAgent() {
  return useMutation({
    mutationFn: async ({ question, mode }: { question: string; mode?: string }) => {
      const result = await commands.askAgent(question, mode ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}
```

- [ ] **Step 2: Run tests**

Run: `cd ui && npx vitest run`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/hooks/agent.ts
git commit -m "feat: update useAskAgent hook for deep mode"
```

---

### Task 12: Update CommandPalette for deep mode

**Files:**
- Modify: `ui/src/components/CommandPalette.tsx`

**Interfaces:**
- Consumes: Updated `useAskAgent` from Task 11
- Produces: Deep think toggle and trace/changes rendering

- [ ] **Step 1: Add deep think toggle and update AgentAnswer rendering**

Modify `ui/src/components/CommandPalette.tsx` to:
- Add a "Deep think" toggle near the ask input
- Pass `mode` to `useAskAgent`
- Render `trace` and `changes` from the response

- [ ] **Step 2: Run tests**

Run: `cd ui && npx vitest run src/components/CommandPalette.test.tsx`
Expected: Tests pass.

- [ ] **Step 3: Commit**

```bash
git add ui/src/components/CommandPalette.tsx
git commit -m "feat: add deep think toggle to CommandPalette"
```

---

### Task 13: Update Copilot screen for deep mode

**Files:**
- Modify: `ui/src/screens/Copilot.tsx`

**Interfaces:**
- Consumes: Updated `useAskAgent` from Task 11
- Produces: Deep think toggle and trace/changes rendering in Copilot

- [ ] **Step 1: Add deep think toggle and update ask flow**

Modify `ui/src/screens/Copilot.tsx` to:
- Add a "Deep think" toggle in the input bar area
- Store toggle state in localStorage
- Call `askAgent` with mode when submitting
- Render the new `AgentAnswer` with trace pill and changes card

- [ ] **Step 2: Run tests**

Run: `cd ui && npx vitest run src/screens/Copilot.test.tsx`
Expected: Tests pass.

- [ ] **Step 3: Commit**

```bash
git add ui/src/screens/Copilot.tsx
git commit -m "feat: add deep think mode to Copilot screen"
```

---

### Task 14: End-to-end manual test

**Files:** None (manual verification)

- [ ] **Step 1: Run dev server**

Run: `pnpm tauri:dev`

- [ ] **Step 2: Load demo data**

Go to Settings → click "Load demo data" (DEV mode)

- [ ] **Step 3: Test simple question**

Open CommandPalette (⌘K), type "hi", verify fast response without tools.

- [ ] **Step 4: Test deep question**

Type "I just got my pay of $4k. What should I do with it first?" with Deep think ON, verify:
- Trace pill shows tools used
- Response includes reasoning
- Changes card shows any goal updates or planned transactions

- [ ] **Step 5: Verify DB changes**

Check that planned_transactions table has new rows if agent created them.

---

### Task 15: Run full test suite

**Files:** None (verification)

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 2: Run all frontend tests**

Run: `cd ui && npx vitest run`
Expected: All tests pass.

- [ ] **Step 3: TypeScript check**

Run: `cd ui && npx tsc --noEmit`
Expected: No TypeScript errors.

- [ ] **Step 4: Final commit if needed**

```bash
git add -A
git commit -m "chore: final cleanup and test fixes"
```
