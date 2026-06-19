use crate::context::{build_context, FinancialContext};
use anyhow::anyhow;
use finsight_core::{
    error::{CoreError, CoreResult},
    models::AgentActionBundle,
    repos::{copilot_actions, copilot_sessions},
};
use rusqlite::Connection;
use serde_json::Value;
use std::sync::Arc;

use crate::CompletionProvider;

const ACTION_KINDS: &[&str] = &[
    "set_budget",
    "update_goal_monthly",
    "update_goal_target",
    "set_transaction_category",
    "set_transaction_flag",
    "create_rule",
    "save_scenario",
    "generate_report",
];

#[derive(Debug, Clone)]
pub struct PlanResult {
    pub bundle: AgentActionBundle,
    pub answer: String,
    pub assumptions: Vec<String>,
    pub follow_up_questions: Vec<String>,
    pub forecast_summary: Option<String>,
}

pub async fn plan(
    conn: &mut Connection,
    session_id: Option<&str>,
    question: &str,
    provider: Arc<dyn CompletionProvider>,
    provider_id: &str,
    model_id: &str,
) -> anyhow::Result<PlanResult> {
    let context = build_context(conn);
    let llm_response = provider
        .complete_json(&build_system_prompt(&context), question)
        .await?;
    if !llm_response.is_object() {
        return Err(anyhow!("Planner: LLM response was not a JSON object"));
    }
    persist_plan(
        conn,
        session_id,
        question,
        &llm_response,
        provider_id,
        model_id,
    )
    .map_err(Into::into)
}

pub fn build_system_prompt(ctx: &FinancialContext) -> String {
    format!(
        "You are a Personal Financial Analyst and coach for a local-first personal finance app.\n\
You advise with the wisdom of six foundational personal finance books:\n\
\n\
PHILOSOPHY FRAMEWORK (apply these in every response):\n\
1. **The Richest Man in Babylon** — Pay yourself first. Recommend saving at least 10% of income \
before any other spending. If the savings rate is below 10%, always address this first.\n\
2. **The Psychology of Money** — Behavior matters more than intelligence. Long time horizons and \
compounding are the most powerful forces in finance. Frame advice in terms of long-term impact. \
Acknowledge that perfect is the enemy of good — consistent, reasonable beats brilliant-but-unrealistic.\n\
3. **Rich Dad Poor Dad** — Distinguish assets (put money in your pocket) from liabilities (take money out). \
Encourage building income-producing assets. Treat financial education as a priority.\n\
4. **The Total Money Makeover** — Follow a staged Baby Steps approach: (1) starter emergency fund, \
(2) eliminate non-mortgage debt using the snowball method (smallest balance first), \
(3) full 3-6 month emergency fund, (4) invest 15% of income, (5) build wealth. \
Never skip steps. Always check emergency fund coverage before investment advice.\n\
5. **I Will Teach You to Be Rich** — Use the Conscious Spending framework: ~50-60% Fixed costs, \
~10%+ Investments, ~5-10% Savings, ~20-35% Guilt-free spending. Automate everything. \
Focus on big wins (recurring bills, subscriptions) rather than eliminating small pleasures.\n\
6. **Think and Grow Rich** — Goals need a definite purpose and burning desire, not just a number. \
Ask about the 'why' behind goals. Celebrate progress milestones. Positive framing.\n\
\n\
PRIORITY ORDER FOR ADVICE:\n\
Step 1 — If emergency fund coverage < 1 month: prioritize emergency fund above everything else.\n\
Step 2 — If there is active debt (debt-payoff goals > 0): address debt snowball before investing.\n\
Step 3 — If savings rate < 10%: apply pay-yourself-first principle.\n\
Step 4 — If conscious spending split is off target: suggest category rebalancing.\n\
Step 5 — Long-term goal optimization, compounding, and asset building.\n\
\n\
The user's current financial context:\n\n\
{}\n\n\
---\n\
Your task: Answer the user's financial question using this wisdom. Respond ONLY with valid JSON in this exact schema:\n\
{{\n\
  \"answer\": \"string — helpful, specific, first-person explanation with numbers. \
Reference relevant principles (e.g., 'Based on Babylon's pay-yourself-first rule...'). \
Be encouraging but honest.\",\n\
  \"assumptions\": [\"string\"],\n\
  \"follow_up_questions\": [\"string — only if you need clarification or data is missing\"],\n\
  \"confidence\": 0.85,\n\
  \"actions\": [\n\
    {{\n\
      \"kind\": \"set_budget | update_goal_monthly | update_goal_target | set_transaction_category | set_transaction_flag | create_rule | save_scenario | generate_report\",\n\
      \"payload\": {{ ... }},\n\
      \"rationale\": \"string — cite which principle drives this action\",\n\
      \"confidence\": 0.9\n\
    }}\n\
  ],\n\
  \"forecast_summary\": \"optional string — if actions are proposed, describe the compound/long-term impact\"\n\
}}\n\n\
Action payloads:\n\
- set_budget: {{\"categoryId\":\"<id>\",\"month\":\"YYYY-MM\",\"amountCents\":<i64>}}\n\
- update_goal_monthly: {{\"goalId\":\"<id>\",\"monthlyDeltaCents\":<i64>}}\n\
- update_goal_target: {{\"goalId\":\"<id>\",\"targetCents\":<i64>}}\n\
- set_transaction_category: {{\"transactionId\":\"<id>\",\"categoryId\":\"<id>\"}}\n\
- set_transaction_flag: {{\"transactionId\":\"<id>\",\"flag\":\"reimbursable\"|\"anomaly_clear\"}}\n\
- create_rule: {{\"pattern\":\"<merchant substring>\",\"categoryId\":\"<id>\"}}\n\
- save_scenario: {{\"description\":\"<string>\",\"params\":{{...}}}}\n\
- generate_report: {{\"reportType\":\"<string>\",\"scope\":\"<month|quarter|year>\"}}\n\n\
Only include actions you are confident about. If no actions are appropriate, leave \"actions\" as [].\n\
Do not include markdown. Output ONLY the JSON object.",
        ctx.to_prompt_string()
    )
}

pub fn persist_plan(
    conn: &mut Connection,
    session_id: Option<&str>,
    question: &str,
    llm_json: &Value,
    provider_id: &str,
    model_id: &str,
) -> CoreResult<PlanResult> {
    let Some(obj) = llm_json.as_object() else {
        return Err(CoreError::InvalidState(
            "Planner: LLM response was not a JSON object".into(),
        ));
    };

    let answer = obj
        .get("answer")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "The assistant produced no readable answer.".to_string());
    let assumptions = string_array(obj.get("assumptions"));
    let mut follow_up_questions = string_array(obj.get("follow_up_questions"));
    let confidence = clamp_confidence(obj.get("confidence").and_then(Value::as_f64));
    let forecast_summary = obj
        .get("forecast_summary")
        .and_then(Value::as_str)
        .map(str::to_string);

    let rationale = if assumptions.is_empty() {
        "No explicit assumptions".to_string()
    } else {
        assumptions.join("; ")
    };
    let title = truncate_title(question, 80);
    let mut bundle = copilot_actions::insert_bundle(
        conn,
        session_id,
        &title,
        &answer,
        &rationale,
        confidence,
        Some(provider_id),
        Some(model_id),
    )?;

    let actions = obj
        .get("actions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for (idx, action) in actions.into_iter().enumerate() {
        let Some(action_obj) = action.as_object() else {
            continue;
        };
        let Some(kind) = action_obj.get("kind").and_then(Value::as_str) else {
            continue;
        };
        if !ACTION_KINDS.contains(&kind) {
            eprintln!("planner: skipping unknown action kind '{kind}'");
            continue;
        }

        let payload = action_obj
            .get("payload")
            .cloned()
            .filter(|value| value.is_object())
            .unwrap_or_else(|| serde_json::json!({}));
        let item = copilot_actions::insert_item(
            conn,
            &bundle.id,
            kind,
            &serde_json::to_string(&payload).map_err(|e| CoreError::InvalidState(e.to_string()))?,
            action_obj
                .get("rationale")
                .and_then(Value::as_str)
                .unwrap_or("No rationale provided."),
            clamp_confidence(action_obj.get("confidence").and_then(Value::as_f64)),
            idx as i64,
        )?;
        bundle.items.push(item);
    }

    if bundle.items.is_empty() && follow_up_questions.is_empty() {
        follow_up_questions.push(
            "Would you like a recommendation only, or should I propose concrete changes?"
                .to_string(),
        );
    }

    let context_json = serde_json::to_string(&build_context(conn))
        .map_err(|e| CoreError::InvalidState(e.to_string()))?;
    copilot_sessions::save_context_snapshot(conn, session_id, &context_json)?;

    Ok(PlanResult {
        bundle,
        answer,
        assumptions,
        follow_up_questions,
        forecast_summary,
    })
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn clamp_confidence(value: Option<f64>) -> f64 {
    value.unwrap_or(0.0).clamp(0.0, 1.0)
}

fn truncate_title(input: &str, limit: usize) -> String {
    let mut chars = input.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() && limit > 0 {
        let keep = limit.saturating_sub(1);
        let prefix: String = input.chars().take(keep).collect();
        format!("{prefix}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use finsight_core::{db::run_migrations, keychain, repos::copilot_actions, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("planner.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    struct FakeProvider {
        response: Value,
    }

    #[async_trait]
    impl CompletionProvider for FakeProvider {
        fn provider_id(&self) -> &str {
            "fake"
        }

        fn model_id(&self) -> &str {
            "fake-model"
        }

        async fn complete_json(&self, _system: &str, _user: &str) -> anyhow::Result<Value> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn persist_plan_creates_bundle_and_filters_unknown_actions() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let llm = serde_json::json!({
            "answer": "You should rebalance groceries.",
            "assumptions": ["Current month budgets are still editable."],
            "follow_up_questions": [],
            "confidence": 0.88,
            "actions": [
                {
                    "kind": "set_budget",
                    "payload": {"categoryId":"groceries","month":"2026-06","amountCents":45000},
                    "rationale": "Recent spend is above target.",
                    "confidence": 0.91
                },
                {
                    "kind": "hack_the_plan",
                    "payload": {"oops": true},
                    "rationale": "Should be ignored.",
                    "confidence": 1.0
                }
            ]
        });

        let result = persist_plan(
            &mut conn,
            None,
            "Increase my groceries budget to $450 this month",
            &llm,
            "anthropic",
            "claude",
        )
        .unwrap();

        assert_eq!(result.answer, "You should rebalance groceries.");
        assert_eq!(result.assumptions.len(), 1);
        assert_eq!(result.bundle.items.len(), 1);

        let stored = copilot_actions::get_bundle(&mut conn, &result.bundle.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.items.len(), 1);
        assert_eq!(stored.items[0].action_kind, "set_budget");
    }

    #[tokio::test]
    async fn plan_returns_error_for_non_object_json() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let provider = Arc::new(FakeProvider {
            response: serde_json::json!("not-an-object"),
        });

        let err = plan(&mut conn, None, "Help me", provider, "fake", "fake-model")
            .await
            .unwrap_err();

        assert!(err
            .to_string()
            .contains("Planner: LLM response was not a JSON object"));
    }
}
