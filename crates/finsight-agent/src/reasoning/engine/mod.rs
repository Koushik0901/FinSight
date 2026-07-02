use crate::reasoning::messages::{
    AgentChange, AssistantTurn, ChatMessage, ReasoningResult, ToolCall,
};
use crate::reasoning::tools::{ToolContext, ToolSet};
use crate::CompletionProvider;
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

pub struct ReasoningEngine;

#[derive(Debug, Clone)]
pub enum ReasoningEngineEvent {
    ToolCallStart {
        call: ToolCall,
    },
    ToolCallResult {
        tool_call_id: String,
        tool_name: String,
        result: Value,
        is_error: bool,
    },
}

impl ReasoningEngine {
    pub async fn run(
        conn: &mut rusqlite::Connection,
        question: &str,
        tools: &ToolSet,
        provider: Arc<dyn CompletionProvider>,
        max_iterations: usize,
    ) -> Result<ReasoningResult> {
        Self::run_with_events(conn, question, tools, provider, max_iterations, |_| {}).await
    }

    pub async fn run_with_events(
        conn: &mut rusqlite::Connection,
        question: &str,
        tools: &ToolSet,
        provider: Arc<dyn CompletionProvider>,
        max_iterations: usize,
        mut on_event: impl FnMut(ReasoningEngineEvent),
    ) -> Result<ReasoningResult> {
        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage::System {
                content: Self::build_system_prompt(tools),
            },
            ChatMessage::User {
                content: question.to_string(),
            },
        ];
        let mut trace: Vec<String> = Vec::new();
        let mut changes: Vec<AgentChange> = Vec::new();
        let mut draft_actions = Vec::new();

        for _ in 0..max_iterations {
            let turn = provider
                .complete_tool_turn(&messages, &tools.definitions())
                .await?;

            match turn {
                AssistantTurn::ToolCalls(calls) => {
                    let mut tool_result_msgs = Vec::new();
                    for call in &calls {
                        trace.push(format!("Called tool: {}", call.name));
                        on_event(ReasoningEngineEvent::ToolCallStart { call: call.clone() });
                        let mut ctx = ToolContext {
                            conn,
                            changes: &mut changes,
                            draft_actions: &mut draft_actions,
                        };
                        let result =
                            tools.execute_recoverable(&call.name, &mut ctx, call.arguments.clone());
                        if result.had_error {
                            trace.push(format!("Tool error: {}", call.name));
                        }
                        on_event(ReasoningEngineEvent::ToolCallResult {
                            tool_call_id: call.id.clone(),
                            tool_name: call.name.clone(),
                            result: result.value.clone(),
                            is_error: result.had_error,
                        });
                        tool_result_msgs.push(ChatMessage::Tool {
                            tool_call_id: call.id.clone(),
                            content: result.value.to_string(),
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
                    return Ok(Self::parse_final_answer(
                        content,
                        reasoning,
                        trace,
                        changes,
                        draft_actions,
                    ));
                }
            }
        }

        Ok(ReasoningResult {
            content: "I analyzed your finances but ran out of reasoning steps. Here's what I found so far.".to_string(),
            reasoning: "The question was too complex for the iteration limit.".to_string(),
            trace,
            changes,
            draft_actions,
            assumptions: Vec::new(),
            data_sources: Vec::new(),
            missing_data: Vec::new(),
            follow_up_questions: Vec::new(),
            response_blocks: Vec::new(),
        })
    }

    fn parse_final_answer(
        content: String,
        reasoning: String,
        trace: Vec<String>,
        changes: Vec<AgentChange>,
        draft_actions: Vec<crate::reasoning::messages::AgentDraftAction>,
    ) -> ReasoningResult {
        let Some(parsed) = parse_structured_final_answer(&content) else {
            return ReasoningResult {
                content,
                reasoning,
                trace,
                changes,
                draft_actions,
                assumptions: Vec::new(),
                data_sources: Vec::new(),
                missing_data: Vec::new(),
                follow_up_questions: Vec::new(),
                response_blocks: Vec::new(),
            };
        };

        let mut reasoning_parts = Vec::new();
        if !parsed.reasoning.trim().is_empty() {
            reasoning_parts.push(parsed.reasoning);
        }
        if !reasoning.trim().is_empty() {
            reasoning_parts.push(reasoning);
        }

        ReasoningResult {
            content: parsed.answer,
            reasoning: reasoning_parts.join(" "),
            trace,
            changes,
            draft_actions,
            assumptions: parsed.assumptions,
            data_sources: parsed.data_sources,
            missing_data: parsed.missing_data,
            follow_up_questions: parsed.follow_up_questions,
            response_blocks: parsed.response_blocks,
        }
    }

    fn build_system_prompt(tools: &ToolSet) -> String {
        let tool_defs = tools.definitions();
        let tool_list: String = tool_defs
            .iter()
            .map(|t| {
                format!(
                    "- {}: {} Parameters: {}",
                    t.name, t.description, t.parameters
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "You are a personal financial analyst for a local-first personal finance app.\n\
             You have access to the following tools:\n{}\n\n\
             Always use tools before answering financial planning questions. Start with get_financial_snapshot unless a narrower deterministic tool is clearly sufficient.\n\
             For paycheck or windfall allocation questions, call analyze_cash_inflow.\n\
             For goal timing questions, call calculate_goal_eta.\n\
             For debt ranking questions, call rank_debt_payoff; for payoff timelines or extra-payment comparisons, call run_debt_payoff_scenarios.\n\
             For multi-goal allocation questions, call run_goal_allocation_scenarios.\n\
             For emergency fund targets, liquidity runway, or income-loss questions, call run_emergency_fund_scenarios.\n\
             For savings-vs-debt tradeoff questions, call compare_debt_vs_goal.\n\
             For affordability, runway, monthly-surplus, or month-by-month balance questions, call run_cashflow_projection or run_cashflow_timeline.\n\
             For data sufficiency concerns, call get_data_quality_report.\n\
             When asked about investing, keep the answer principles-only; do not recommend tickers, ETFs, or market timing.\n\
             If emergency coverage is below one month or APR/minimum payment data is missing, say the answer is provisional and ask for the missing information.\n\
             When making a recommendation, compare at least two reasonable alternatives unless the answer is a simple fact lookup.\n\
             The final answer MUST be a JSON object with this schema: {{\"answer\":\"string\", \"reasoning\":\"string\", \"assumptions\":[\"string\"], \"data_sources\":[\"string\"], \"missing_data\":[\"string\"], \"follow_up_questions\":[\"string\"], \"response_blocks\":[...]}}.\n\
             The answer string may use concise Markdown for headings, bullets, tables, and code-style labels because the UI renders Streamdown markdown. Keep it readable while streaming.\n\
             Do not duplicate structured blocks in prose. Use response_blocks only when a visual block makes the answer clearer than prose alone; leave it empty for simple fact answers or short explanations.\n\
             Supported response_blocks are exactly: {{\"kind\":\"markdown\",\"markdown\":\"...\"}}, {{\"kind\":\"table\",\"title\":\"...\",\"columns\":[\"...\"],\"rows\":[[\"...\"]]}}, {{\"kind\":\"barChart\",\"title\":\"...\",\"seriesLabel\":\"...\",\"data\":[{{\"label\":\"...\",\"value\":123}}]}}, {{\"kind\":\"lineChart\",\"title\":\"...\",\"seriesLabel\":\"...\",\"data\":[{{\"label\":\"...\",\"value\":123}}]}}, {{\"kind\":\"metricGrid\",\"metrics\":[{{\"label\":\"...\",\"value\":\"...\",\"detail\":\"...\",\"tone\":\"neutral\"}}]}}, {{\"kind\":\"callout\",\"tone\":\"info\",\"title\":\"...\",\"body\":\"...\"}}.\n\
             Use metricGrid for 2-6 headline numbers, table for alternatives/debt payoff/transaction review rows, barChart for category comparisons, lineChart for time trends, callout for missing-data/risk/next-action warnings, and markdown only for a short supplemental section that should be visually separated.\n\
             Never output arbitrary HTML, arbitrary React/component names, executable props, unvalidated URLs, or blocks outside the supported list.\n\
             The answer string must include recommendation, numbers used, alternatives compared, assumptions, missing data, and next action when those apply.\n\
             Be specific with numbers. Explain your reasoning clearly and cite which local data/tool result you used in data_sources.\n\
             Autonomous actions (update_goal_monthly, create_planned_transaction) are allowed only as draft actions that still require user approval.\n\
             If a tool result returns {{\"ok\":false}}, inspect the error message, fix the tool name or arguments, and retry when retryable.\n\
             Respond with either tool calls or the final JSON object. Do not wrap final JSON in markdown fences.", tool_list
        )
    }
}

#[derive(Debug, Deserialize)]
struct StructuredFinalAnswer {
    answer: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    assumptions: Vec<String>,
    #[serde(default)]
    data_sources: Vec<String>,
    #[serde(default)]
    missing_data: Vec<String>,
    #[serde(default)]
    follow_up_questions: Vec<String>,
    #[serde(default)]
    response_blocks: Vec<Value>,
}

fn parse_structured_final_answer(content: &str) -> Option<StructuredFinalAnswer> {
    let trimmed = content.trim();
    if let Ok(answer) = serde_json::from_str::<StructuredFinalAnswer>(trimmed) {
        return Some(answer);
    }

    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end < start {
        return None;
    }
    serde_json::from_str::<StructuredFinalAnswer>(&trimmed[start..=end]).ok()
}

#[cfg(test)]
mod tests;
