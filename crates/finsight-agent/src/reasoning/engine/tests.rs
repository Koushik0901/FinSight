use super::ReasoningEngine;
use crate::reasoning::messages::{AssistantTurn, ToolCall};
use crate::reasoning::tools::{ToolSet, read, act};
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
    let result = ReasoningEngine::run(&mut *conn, "What is my savings rate?", &tools, provider, 5).await.unwrap();
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
    let result = ReasoningEngine::run(&mut *conn, "What are my account balances?", &tools, provider, 5).await.unwrap();
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
    let result = ReasoningEngine::run(&mut *conn, "Complex question", &tools, provider, 2).await.unwrap();
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
    let result = ReasoningEngine::run(&mut *conn, "Increase my invest goal", &tools, provider, 5).await.unwrap();
    assert_eq!(result.changes.len(), 1);
    assert_eq!(result.changes[0].kind, "goal");
}
