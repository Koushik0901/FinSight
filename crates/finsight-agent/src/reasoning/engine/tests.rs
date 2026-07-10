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
async fn hitting_the_time_budget_synthesizes_a_best_effort_answer() {
    // A heavy question must never hard-fail: once the wall-clock budget is spent,
    // the loop forces a final synthesis turn and returns a real answer instead of
    // looping to exhaustion and shipping the canned non-answer.
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(MockCompletionProvider {
        provider_id: "mock".into(),
        model_id: "test".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![
            // Turn 0: a tool call, so the loop advances past iteration 0 with
            // some data gathered.
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "c1".into(),
                    name: "get_account_balances".into(),
                    arguments: json!({}),
                }],
                plan: None,
            },
            // The forced time-limited synthesis turn returns a real answer.
            AssistantTurn::FinalAnswer {
                content: "Here's my best-effort read from the balances I gathered.".to_string(),
                reasoning: String::new(),
            },
        ]),
    });
    let tools = build_toolset();
    // Deadline already reached → synthesize on iteration 1.
    let result = ReasoningEngine::run_with_events(
        &mut *conn,
        "Give me a full financial plan",
        &tools,
        provider,
        None,
        10,
        Some(std::time::Instant::now()),
        |_| {},
    )
    .await
    .unwrap();

    assert!(
        result.content.contains("best-effort read"),
        "should return the synthesized answer, got: {}",
        result.content
    );
    assert!(
        !result.content.contains("ran out of reasoning steps"),
        "must not return the canned iteration-exhaustion non-answer"
    );
    assert!(
        result.trace.iter().any(|t| t.contains("Time budget")),
        "trace should record the time-budget synthesis"
    );
    assert!(
        result.hit_time_budget,
        "hit_time_budget must be set so the caller can kick off a background deep answer"
    );
}

#[tokio::test]
async fn strong_synthesizer_rewrites_the_final_answer_after_tool_gathering() {
    // Model tiers: the cheap router drives tool selection, the strong synthesizer
    // writes the answer the user sees. After a tool runs, the router's draft is
    // replaced by the synthesizer's answer.
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let router = Arc::new(MockCompletionProvider {
        provider_id: "router".into(),
        model_id: "fast".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "c1".into(),
                    name: "get_account_balances".into(),
                    arguments: json!({}),
                }],
                plan: None,
            },
            AssistantTurn::FinalAnswer {
                content: "router draft answer".to_string(),
                reasoning: String::new(),
            },
        ]),
    });
    let synthesizer = Arc::new(MockCompletionProvider {
        provider_id: "synth".into(),
        model_id: "strong".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![AssistantTurn::FinalAnswer {
            content: "STRONG synthesized final answer".to_string(),
            reasoning: String::new(),
        }]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run_with_events(
        &mut *conn,
        "What are my balances?",
        &tools,
        router,
        Some(synthesizer),
        5,
        None,
        |_| {},
    )
    .await
    .unwrap();

    assert!(
        result.content.contains("STRONG synthesized"),
        "the strong synthesizer should write the final answer, got: {}",
        result.content
    );
    assert!(!result.content.contains("router draft"));
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
        None,
        5,
        None,
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

#[tokio::test]
async fn run_with_events_ignores_a_plan_offered_on_a_later_turn() {
    // The system prompt tells the model to only emit a PLAN: preamble once,
    // on its first turn — but the engine must not trust that blindly. If a
    // later turn's AssistantTurn::ToolCalls also carries Some(plan) (e.g. a
    // model that ignores the instruction and repeats it), the engine must
    // still only ever fire one PlanReady, from the first turn.
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
                plan: Some(vec!["First-turn step".to_string()]),
            },
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "call-2".to_string(),
                    name: "get_account_balances".to_string(),
                    arguments: json!({}),
                }],
                plan: Some(vec!["Second-turn step that should be ignored".to_string()]),
            },
            AssistantTurn::FinalAnswer {
                content: r#"{"answer":"Done.","reasoning":"","assumptions":[],"data_sources":[],"missing_data":[],"follow_up_questions":[],"response_blocks":[]}"#.to_string(),
                reasoning: String::new(),
            },
        ]),
    });
    let tools = build_toolset();

    let mut events = Vec::new();
    let result = ReasoningEngine::run_with_events(
        &mut conn,
        "What's my net worth?",
        &tools,
        provider,
        None,
        5,
        None,
        |event| events.push(event),
    )
    .await
    .unwrap();

    let plan_ready_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, ReasoningEngineEvent::PlanReady { .. }))
        .collect();
    assert_eq!(
        plan_ready_events.len(),
        1,
        "expected exactly one PlanReady event, even though two turns offered a plan"
    );
    if let ReasoningEngineEvent::PlanReady { steps } = plan_ready_events[0] {
        assert_eq!(steps, &vec!["First-turn step".to_string()]);
    }
    assert_eq!(result.plan, vec!["First-turn step".to_string()]);
}

#[tokio::test]
async fn plan_only_turn_is_not_accepted_as_final_answer() {
    // The model emits ONLY its PLAN preamble on the first turn (no tools, no
    // JSON). The engine must NOT ship that as the answer — it should nudge the
    // model to continue and use the real answer it produces next.
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(MockCompletionProvider {
        provider_id: "mock".into(),
        model_id: "test".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![
            AssistantTurn::FinalAnswer {
                content: "PLAN:\n1. Fetch net worth\n2. Report it".to_string(),
                reasoning: String::new(),
            },
            AssistantTurn::FinalAnswer {
                content: "Your net worth is -$2,200.".to_string(),
                reasoning: "Computed from accounts.".to_string(),
            },
        ]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "What is my net worth?", &tools, provider, 5)
        .await
        .unwrap();
    assert!(
        result.content.contains("-$2,200"),
        "expected the real second-turn answer, got: {}",
        result.content
    );
    assert!(
        !result.content.contains("PLAN:"),
        "the raw plan preamble must not leak into the answer"
    );
    // The plan itself is still captured for the UI.
    assert_eq!(result.plan, vec!["Fetch net worth".to_string(), "Report it".to_string()]);
}

#[test]
fn content_after_plan_detects_plan_only_vs_real_answer() {
    use super::content_after_plan;
    assert_eq!(content_after_plan("PLAN:\n1. a\n2. b"), "");
    assert_eq!(content_after_plan("PLAN:\n1. a\n\n"), "");
    assert_eq!(
        content_after_plan("PLAN:\n1. a\n2. b\n\nThe answer is 42."),
        "The answer is 42."
    );
    assert_eq!(content_after_plan("Just a plain answer."), "Just a plain answer.");
}

#[tokio::test]
async fn plain_prose_clarification_with_no_tool_call_is_a_real_answer() {
    // The model sometimes answers a quick clarifying question in plain prose
    // instead of the JSON answer contract, and legitimately calls no tool. The
    // engine must mark this a real answer (not a stall) so the app's usability
    // gate doesn't replace a correct clarification with the canned fallback.
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(MockCompletionProvider {
        provider_id: "mock".into(),
        model_id: "test".into(),
        response: json!({}),
        tool_turns: Mutex::new(vec![AssistantTurn::FinalAnswer {
            content: "I'd love to help! What's the purchase and how much does it cost?"
                .to_string(),
            reasoning: String::new(),
        }]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "Can I afford it?", &tools, provider, 5)
        .await
        .unwrap();
    assert!(result.content.contains("What's the purchase"));
    assert!(result.trace.is_empty(), "no tool call expected for a clarifying question");
    assert!(
        result.is_real_answer,
        "plain-prose clarification must be marked a real answer, not a stall"
    );
}

/// Test-only provider that scripts turns like `MockCompletionProvider`, but
/// also counts calls to `complete_tool_turn_forced` — so a test can assert
/// the engine actually asked for a FORCED tool call after a stall, not merely
/// that the scripted turns were consumed in order (which the default
/// delegation would satisfy either way).
struct ForceTrackingProvider {
    tool_turns: Mutex<Vec<AssistantTurn>>,
    forced_calls: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl crate::CompletionProvider for ForceTrackingProvider {
    fn provider_id(&self) -> &str {
        "force-tracking"
    }
    fn model_id(&self) -> &str {
        "test"
    }
    async fn complete_json(&self, _system: &str, _user: &str) -> anyhow::Result<serde_json::Value> {
        Ok(json!({}))
    }
    async fn complete_tool_turn(
        &self,
        _messages: &[crate::reasoning::messages::ChatMessage],
        _tools: &[crate::reasoning::messages::ToolDefinition],
    ) -> anyhow::Result<AssistantTurn> {
        let mut turns = self.tool_turns.lock().unwrap();
        if turns.is_empty() {
            Ok(AssistantTurn::FinalAnswer {
                content: "No more turns scripted".to_string(),
                reasoning: String::new(),
            })
        } else {
            Ok(turns.remove(0))
        }
    }
    async fn complete_tool_turn_forced(
        &self,
        messages: &[crate::reasoning::messages::ChatMessage],
        tools: &[crate::reasoning::messages::ToolDefinition],
    ) -> anyhow::Result<AssistantTurn> {
        self.forced_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.complete_tool_turn(messages, tools).await
    }
}

#[tokio::test]
async fn stall_recovery_forces_a_tool_call_on_the_retry_turn() {
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(ForceTrackingProvider {
        tool_turns: Mutex::new(vec![
            AssistantTurn::FinalAnswer {
                content: "PLAN:\n1. Fetch net worth\n2. Report it".to_string(),
                reasoning: String::new(),
            },
            AssistantTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "c1".to_string(),
                    name: "get_net_worth".to_string(),
                    arguments: json!({}),
                }],
                plan: None,
            },
            AssistantTurn::FinalAnswer {
                content: "Your net worth is -$2,200.".to_string(),
                reasoning: String::new(),
            },
        ]),
        forced_calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "What is my net worth?", &tools, provider.clone(), 5)
        .await
        .unwrap();
    assert!(result.content.contains("-$2,200"));
    assert_eq!(
        provider.forced_calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "expected exactly one forced tool-call turn, right after the stall"
    );
}

/// Test-only provider that fails the first N calls to `complete_tool_turn`
/// with a transient-looking error, then succeeds — simulating a network blip
/// or decode hiccup on an otherwise-healthy conversation.
struct FlakyProvider {
    fails_remaining: std::sync::atomic::AtomicUsize,
    tool_turns: Mutex<Vec<AssistantTurn>>,
}

#[async_trait::async_trait]
impl crate::CompletionProvider for FlakyProvider {
    fn provider_id(&self) -> &str {
        "flaky"
    }
    fn model_id(&self) -> &str {
        "test"
    }
    async fn complete_json(&self, _system: &str, _user: &str) -> anyhow::Result<serde_json::Value> {
        Ok(json!({}))
    }
    async fn complete_tool_turn(
        &self,
        _messages: &[crate::reasoning::messages::ChatMessage],
        _tools: &[crate::reasoning::messages::ToolDefinition],
    ) -> anyhow::Result<AssistantTurn> {
        if self.fails_remaining.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            self.fails_remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            return Err(anyhow::anyhow!("simulated transient decode error"));
        }
        let mut turns = self.tool_turns.lock().unwrap();
        if turns.is_empty() {
            Ok(AssistantTurn::FinalAnswer {
                content: "No more turns scripted".to_string(),
                reasoning: String::new(),
            })
        } else {
            Ok(turns.remove(0))
        }
    }
}

#[tokio::test]
async fn transient_provider_error_is_retried_and_recovers() {
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let provider = Arc::new(FlakyProvider {
        fails_remaining: std::sync::atomic::AtomicUsize::new(2),
        tool_turns: Mutex::new(vec![AssistantTurn::FinalAnswer {
            content: "Your net worth is -$2,200.".to_string(),
            reasoning: String::new(),
        }]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "What is my net worth?", &tools, provider, 5)
        .await
        .expect("2 transient failures should be retried within the 3-attempt budget");
    assert!(result.content.contains("-$2,200"));
}

#[tokio::test]
async fn provider_error_still_propagates_once_retries_are_exhausted() {
    let (_dir, db) = fresh_db();
    let mut conn = db.get().unwrap();
    // Always fails: more failures than the retry budget covers, so the run
    // must still surface an error rather than retry forever.
    let provider = Arc::new(FlakyProvider {
        fails_remaining: std::sync::atomic::AtomicUsize::new(100),
        tool_turns: Mutex::new(vec![]),
    });
    let tools = build_toolset();
    let result = ReasoningEngine::run(&mut *conn, "What is my net worth?", &tools, provider, 5).await;
    assert!(result.is_err(), "a persistently failing provider must still surface an error");
}
