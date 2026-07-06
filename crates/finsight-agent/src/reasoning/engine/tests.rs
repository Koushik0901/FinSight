use super::{ReasoningEngine, ReasoningEngineEvent};
use crate::providers::mock::MockCompletionProvider;
use crate::reasoning::messages::{AssistantTurn, ToolCall};
use crate::reasoning::tools::{act, read, ToolSet};
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
    tools.register(act::set_budget());
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
    let result = ReasoningEngine::run(&mut *conn, "What is my savings rate?", &tools, provider, 5)
        .await
        .unwrap();
    assert!(result.content.contains("20%"));
    assert!(result.trace.is_empty());
}

#[tokio::test]
async fn structured_final_answer_populates_answer_metadata() {
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(MockCompletionProvider {
        provider_id: "mock".into(),
        model_id: "test".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![AssistantTurn::FinalAnswer {
            content: json!({
                "answer": "Recommendation: keep debt payoff first. Numbers used: $500. Alternatives compared: debt vs savings. Assumptions: local data only. Missing data: APR. Next action: update APR.",
                "reasoning": "Used local liabilities and goals.",
                "assumptions": ["Local data only"],
                "data_sources": ["liabilities"],
                "missing_data": ["APR"],
                "follow_up_questions": ["What is the APR?"],
                "response_blocks": [{
                    "kind": "metricGrid",
                    "metrics": [{
                        "label": "Decision",
                        "value": "Debt first",
                        "detail": "APR is missing, so this is provisional.",
                        "tone": "warning"
                    }]
                }]
            })
            .to_string(),
            reasoning: "provider reasoning".to_string(),
        }]),
    });

    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "Should I pay debt?", &tools, provider, 5)
        .await
        .unwrap();

    assert!(result.content.starts_with("Recommendation:"));
    assert_eq!(result.assumptions, vec!["Local data only"]);
    assert_eq!(result.data_sources, vec!["liabilities"]);
    assert_eq!(result.missing_data, vec!["APR"]);
    assert_eq!(result.follow_up_questions, vec!["What is the APR?"]);
    assert_eq!(result.response_blocks.len(), 1);
    assert_eq!(result.response_blocks[0]["kind"], "metricGrid");
    assert!(result.reasoning.contains("Used local liabilities"));
    assert!(result.reasoning.contains("provider reasoning"));
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
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call_1".into(),
                    name: "get_account_balances".into(),
                    arguments: json!({}),
                }],
                plan: None,
            },
            AssistantTurn::FinalAnswer {
                content: "You have $5000 across all accounts".to_string(),
                reasoning: "Summed account balances".to_string(),
            },
        ]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(
        &mut *conn,
        "What are my account balances?",
        &tools,
        provider,
        5,
    )
    .await
    .unwrap();
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
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call_1".into(),
                    name: "get_account_balances".into(),
                    arguments: json!({}),
                }],
                plan: None,
            },
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call_2".into(),
                    name: "get_month_totals".into(),
                    arguments: json!({}),
                }],
                plan: None,
            },
        ]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "Complex question", &tools, provider, 2)
        .await
        .unwrap();
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
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call_1".into(),
                    name: "draft_update_goal_monthly".into(),
                    arguments: json!({"goal_id": "g1", "monthly_delta_cents": 15000}),
                }],
                plan: None,
            },
            AssistantTurn::FinalAnswer {
                content: "Updated your invest goal".to_string(),
                reasoning: "Increased contribution".to_string(),
            },
        ]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "Increase my invest goal", &tools, provider, 5)
        .await
        .unwrap();
    assert_eq!(result.changes.len(), 1);
    assert_eq!(result.changes[0].kind, "draft_action");
    assert_eq!(result.draft_actions.len(), 1);
    assert_eq!(result.draft_actions[0].action_kind, "update_goal_monthly");
}

#[tokio::test]
async fn budget_action_tool_drafts_without_mutating_budget() {
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    conn.execute(
        "INSERT INTO category_groups(id, label, sort_order) VALUES('g1','Essentials',0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO categories(id, group_id, label, color, icon, sort_order) VALUES('cat1','g1','Groceries','#fff','cart',0)",
        [],
    )
    .unwrap();
    let provider = Arc::new(MockCompletionProvider {
        provider_id: "mock".into(),
        model_id: "test".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call_budget".into(),
                    name: "draft_set_budget".into(),
                    arguments: json!({"category_id":"cat1","month":"2026-06","amount_cents":65000,"rationale":"Groceries are trending higher."}),
                }],
                plan: None,
            },
            AssistantTurn::FinalAnswer {
                content: "Drafted a grocery budget change for approval.".to_string(),
                reasoning: "Budget action is a draft only.".to_string(),
            },
        ]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "Draft a grocery budget", &tools, provider, 5)
        .await
        .unwrap();

    assert_eq!(result.draft_actions.len(), 1);
    assert_eq!(result.draft_actions[0].action_kind, "set_budget");
    assert!(result.draft_actions[0].payload_json.contains("amountCents"));
    assert_eq!(result.changes[0].kind, "draft_action");
    let budget_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM budgets", [], |r| r.get(0))
        .unwrap();
    assert_eq!(budget_count, 0);
}

#[tokio::test]
async fn invalid_tool_call_returns_recoverable_error_then_recovers() {
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(MockCompletionProvider {
        provider_id: "mock".into(),
        model_id: "test".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call_bad".into(),
                    name: "get_top_spending_categories".into(),
                    arguments: json!({"limit": "five"}),
                }],
                plan: None,
            },
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call_good".into(),
                    name: "get_top_spending_categories".into(),
                    arguments: json!({"limit": 5}),
                }],
                plan: None,
            },
            AssistantTurn::FinalAnswer {
                content: "Recovered after correcting the limit argument.".to_string(),
                reasoning: "The first tool result returned a recoverable argument error."
                    .to_string(),
            },
        ]),
    });
    let mut tools = build_toolset();
    tools.register(read::get_top_spending_categories());

    let result = ReasoningEngine::run(&mut *conn, "Top categories", &tools, provider, 5)
        .await
        .unwrap();

    assert!(result.content.contains("Recovered"));
    assert!(result
        .trace
        .iter()
        .any(|t| t == "Tool error: get_top_spending_categories"));
    assert!(result
        .trace
        .iter()
        .any(|t| t == "Called tool: get_top_spending_categories"));
}

#[tokio::test]
async fn unknown_tool_call_returns_recoverable_error() {
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(MockCompletionProvider {
        provider_id: "mock".into(),
        model_id: "test".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call_unknown".into(),
                    name: "not_a_real_tool".into(),
                    arguments: json!({}),
                }],
                plan: None,
            },
            AssistantTurn::FinalAnswer {
                content: "I retried with an available tool.".to_string(),
                reasoning: "The unknown tool error was recoverable.".to_string(),
            },
        ]),
    });
    let tools = build_toolset();

    let result = ReasoningEngine::run(&mut *conn, "Use a bad tool", &tools, provider, 5)
        .await
        .unwrap();

    assert!(result.content.contains("retried"));
    assert!(result
        .trace
        .iter()
        .any(|t| t == "Tool error: not_a_real_tool"));
}

#[tokio::test]
async fn run_with_events_emits_plan_ready_before_any_tool_call() {
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(MockCompletionProvider {
        provider_id: "mock".into(),
        model_id: "test".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "get_account_balances".to_string(),
                    arguments: json!({}),
                }],
                plan: Some(vec![
                    "Find the income that just landed".to_string(),
                    "Rank every debt by interest rate".to_string(),
                ]),
            },
            AssistantTurn::FinalAnswer {
                content: r#"{"answer":"Done.","reasoning":"","assumptions":[],"data_sources":[],"missing_data":[],"follow_up_questions":[],"response_blocks":[]}"#.to_string(),
                reasoning: String::new(),
            },
        ]),
    });
    let tools = build_toolset();

    let mut events = Vec::new();
    let _ = ReasoningEngine::run_with_events(
        &mut conn,
        "What's my net worth?",
        &tools,
        provider,
        5,
        |event| events.push(event),
    )
    .await
    .unwrap();

    let plan_index = events
        .iter()
        .position(|e| matches!(e, ReasoningEngineEvent::PlanReady { .. }));
    let tool_start_index = events
        .iter()
        .position(|e| matches!(e, ReasoningEngineEvent::ToolCallStart { .. }));
    assert!(plan_index.is_some(), "expected a PlanReady event");
    assert!(tool_start_index.is_some(), "expected a ToolCallStart event");
    assert!(
        plan_index.unwrap() < tool_start_index.unwrap(),
        "plan must be emitted before the first tool call"
    );

    if let Some(ReasoningEngineEvent::PlanReady { steps }) = events
        .into_iter()
        .find(|e| matches!(e, ReasoningEngineEvent::PlanReady { .. }))
    {
        assert_eq!(
            steps,
            vec![
                "Find the income that just landed".to_string(),
                "Rank every debt by interest rate".to_string(),
            ]
        );
    } else {
        panic!("expected PlanReady event with steps");
    }
}
