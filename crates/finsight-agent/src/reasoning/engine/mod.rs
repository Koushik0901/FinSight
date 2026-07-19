use crate::reasoning::messages::{
    parse_plan_preamble, AgentChange, AssistantTurn, ChatMessage, ReasoningResult, ToolCall,
};
use crate::reasoning::tools::{ToolContext, ToolSet};
use crate::{CompletionProvider, TurnUsage};
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

pub struct ReasoningEngine;

/// Time reserved for one final synthesis turn before a run's deadline. A single
/// provider turn can take up to its HTTP timeout (60s), so this keeps the
/// synthesis — and any in-flight/failed turn near the wall — from overrunning
/// the caller's outer timeout.
const SYNTHESIS_HEADROOM: Duration = Duration::from_secs(55);

/// Bounded retry around a single provider turn. A cloud LLM call is an
/// unreliable I/O boundary — rate-limit blips and response-decode hiccups are
/// common enough that the eval harness already retries a whole run over them;
/// production had no equivalent and a transient error mid-conversation just
/// failed the request outright, discarding every tool call already made this
/// turn. Retries the SAME turn (not the whole run) with a short backoff, so a
/// hiccup recovers without re-doing prior work; a genuinely broken call
/// (bad auth, malformed request) just fails 3x fast and surfaces the same
/// error a few hundred ms later.
async fn call_provider_with_retry(
    provider: &Arc<dyn CompletionProvider>,
    messages: &[ChatMessage],
    tool_defs: &[crate::reasoning::messages::ToolDefinition],
    forced: bool,
    deadline: Option<std::time::Instant>,
) -> Result<(AssistantTurn, TurnUsage)> {
    const MAX_ATTEMPTS: u32 = 3;
    for attempt in 1..=MAX_ATTEMPTS {
        let result = if forced {
            provider
                .complete_tool_turn_forced_with_usage(messages, tool_defs)
                .await
        } else {
            provider
                .complete_tool_turn_with_usage(messages, tool_defs)
                .await
        };
        match result {
            Ok(turn) => return Ok(turn),
            // Don't burn the whole wall retrying a slow/erroring turn: if we're
            // within a synthesis headroom of the deadline, surface the error now
            // so the loop's budget check can synthesize a best-effort answer
            // instead of a hard timeout. (Retries×HTTP-timeout can otherwise eat
            // the entire budget before the top-of-loop check ever fires.)
            Err(_)
                if attempt < MAX_ATTEMPTS
                    && !deadline
                        .is_some_and(|d| std::time::Instant::now() + SYNTHESIS_HEADROOM >= d) =>
            {
                tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("loop always returns on the final attempt")
}

#[derive(Debug, Clone)]
pub enum ReasoningEngineEvent {
    PlanReady {
        steps: Vec<String>,
    },
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
        Self::run_with_events(
            conn,
            question,
            tools,
            provider,
            None,
            max_iterations,
            None,
            |_| {},
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_with_events(
        conn: &mut rusqlite::Connection,
        question: &str,
        tools: &ToolSet,
        // The tool-selection "router" — used for every gathering turn. In a
        // single-model setup this is the only model.
        provider: Arc<dyn CompletionProvider>,
        // Optional strong "synthesizer": when set, the FINAL answer is (re)written
        // by this model from the gathered tool results, while the cheaper router
        // above drives the many tool-selection turns. `None` = single model.
        synthesizer: Option<Arc<dyn CompletionProvider>>,
        max_iterations: usize,
        // Optional wall-clock budget. When set, the loop stops gathering and
        // synthesizes a best-effort answer once it's within one turn's headroom
        // of the deadline — so a heavy question degrades to a partial answer
        // instead of being hard-killed by the caller's outer timeout with
        // nothing to show. `None` = run to the iteration limit (tests, recipes).
        deadline: Option<std::time::Instant>,
        mut on_event: impl FnMut(ReasoningEngineEvent),
    ) -> Result<ReasoningResult> {
        let mut messages: Vec<ChatMessage> = vec![ChatMessage::System {
            content: Self::build_system_prompt(tools),
        }];
        // Precompute a compact, authoritative financial snapshot into the loop's
        // context so common planning questions can be answered with FEWER tool
        // round-trips (fewer LLM turns = lower latency). The GROUNDING RULE
        // already permits citing "the provided context", so these figures are
        // safe to state directly; the model still calls tools for breakdowns,
        // history, and anything not in the snapshot.
        let snapshot = finance_snapshot_block(conn);
        if !snapshot.is_empty() {
            messages.push(ChatMessage::System { content: snapshot });
        }
        messages.push(ChatMessage::User {
            content: question.to_string(),
        });
        let mut trace: Vec<String> = Vec::new();
        let mut changes: Vec<AgentChange> = Vec::new();
        let mut draft_actions = Vec::new();
        let mut plan: Vec<String> = Vec::new();
        // The best non-empty content the model has produced so far. If the model
        // ends on a non-answer (empty/plan-only) we fall back to this rather than
        // a canned empty string — a plan the user can read beats nothing.
        let mut best_effort_content: Option<String> = None;
        // Whether any tool ran. Gates the strong-synthesizer rewrite: a trivial
        // answer with no tool data is not worth a second (expensive) model call.
        let mut tool_calls_made = false;
        // How many times we've nudged the model to stop stalling. Capped: if one
        // nudge doesn't get it to act, more won't (observed: it can loop forever).
        let mut nudges_used: usize = 0;
        const MAX_NUDGES: usize = 1;
        // Set right after we send the non-answer nudge, so the VERY NEXT turn
        // asks the provider to force a tool call (tool_choice: "required")
        // instead of hoping the model responds to the prose nudge — a
        // deterministic correction beats a polite request it can still ignore.
        let mut force_next_tool_call = false;
        // Token usage summed across every provider turn of this run (tool
        // selection, the strong-synthesizer rewrite, and the time-budget
        // synthesis), stamped onto whichever ReasoningResult we return so the UI
        // can show how much of the prompt the provider served from cache.
        let mut usage_acc = TurnUsage::default();

        for iteration in 0..max_iterations {
            // Wall-clock budget: once we're within a turn's headroom of the
            // deadline, stop gathering and synthesize a best-effort answer from
            // the tool results already in `messages` — never hand the user a
            // hard timeout with nothing. Only after at least one real turn, so
            // the synthesis has something to work from.
            if let Some(d) = deadline {
                if iteration > 0 && std::time::Instant::now() + SYNTHESIS_HEADROOM >= d {
                    trace.push(
                        "Time budget nearly spent — synthesizing a best-effort answer now"
                            .to_string(),
                    );
                    messages.push(ChatMessage::User {
                        content: TIME_LIMIT_SYNTHESIS.to_string(),
                    });
                    let content = match synthesizer
                        .as_ref()
                        .unwrap_or(&provider)
                        .complete_final_answer_turn_with_usage(&messages, &tools.definitions())
                        .await
                    {
                        Ok((AssistantTurn::FinalAnswer { content, .. }, u))
                            if !content.trim().is_empty() =>
                        {
                            usage_acc = usage_acc.saturating_add(u);
                            content
                        }
                        // Synthesis produced tool calls / empty (but still cost
                        // prompt tokens — count them): fall back to the best
                        // content seen, else a trace summary.
                        Ok((_, u)) => {
                            usage_acc = usage_acc.saturating_add(u);
                            best_effort_content
                                .clone()
                                .unwrap_or_else(|| summarize_progress(&trace))
                        }
                        // Errored: nothing to count.
                        Err(_) => best_effort_content
                            .clone()
                            .unwrap_or_else(|| summarize_progress(&trace)),
                    };
                    let mut result = Self::parse_final_answer(
                        content,
                        String::new(),
                        plan,
                        trace,
                        changes,
                        draft_actions,
                    );
                    // Signal the caller to kick off a background deep answer:
                    // this question was heavy enough to nearly time out.
                    result.hit_time_budget = true;
                    result.usage = usage_acc;
                    return Ok(result);
                }
            }

            let forced = force_next_tool_call;
            force_next_tool_call = false;
            let (turn, turn_usage) =
                call_provider_with_retry(&provider, &messages, &tools.definitions(), forced, deadline)
                    .await?;
            usage_acc = usage_acc.saturating_add(turn_usage);

            match turn {
                AssistantTurn::ToolCalls {
                    calls,
                    plan: turn_plan,
                } => {
                    // The system-prompt contract only asks the model for a plan on
                    // its very first response; be defensive and ignore any plan
                    // supplied on later turns even if a provider surfaces one.
                    if iteration == 0 {
                        if let Some(steps) = turn_plan {
                            if !steps.is_empty() {
                                plan = steps.clone();
                                on_event(ReasoningEngineEvent::PlanReady { steps });
                            }
                        }
                    }

                    // NB: tools run SEQUENTIALLY here, deliberately. Parallel
                    // tool calls are a major latency lever for agents whose tools
                    // are remote/network-bound — but FinSight's tools are
                    // synchronous rusqlite reads over a SINGLE `&mut Connection`
                    // (ToolContext.conn), which cannot serve concurrent reads, and
                    // they're local (sub-millisecond), not network. Parallelizing
                    // here buys ~nothing and is blocked by the single-connection
                    // model, so the latency work lives in fewer/faster LLM turns
                    // instead (model tiers, prompt caching, capped max_tokens).
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
                    if !calls.is_empty() {
                        tool_calls_made = true;
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
                    // The model may emit its `PLAN:` preamble as free text on the
                    // first turn (the contract asks for it before anything else).
                    // Capture it so the plan feature still works even when the
                    // plan arrives on a text turn rather than a tool-call turn.
                    if iteration == 0 && plan.is_empty() {
                        if let Some(steps) = parse_plan_preamble(&content) {
                            plan = steps.clone();
                            on_event(ReasoningEngineEvent::PlanReady { steps });
                        }
                    }

                    // A robust agent loop never accepts a non-answer as final.
                    // glm-class models sometimes end a turn having emitted ONLY
                    // the plan (or empty content) with no tool calls — the
                    // provider surfaces that as a FinalAnswer, and returning it
                    // verbatim ships the raw plan (or nothing) to the user. Nudge
                    // the model to act — but only ONCE, and never at the cost of
                    // the answer: track the best content seen and, on give-up,
                    // return that rather than looping to an empty fallback.
                    let has_real_answer = parse_structured_final_answer(&content).is_some()
                        || !content_after_plan(&content).is_empty();
                    if !has_real_answer {
                        if !content.trim().is_empty() {
                            best_effort_content = Some(content.clone());
                        }
                        if nudges_used < MAX_NUDGES && iteration + 1 < max_iterations {
                            nudges_used += 1;
                            trace.push(
                                "Non-answer turn (plan-only/empty) — asked model to continue"
                                    .to_string(),
                            );
                            messages.push(ChatMessage::Assistant {
                                content: Some(content),
                                tool_calls: Vec::new(),
                            });
                            messages.push(ChatMessage::User {
                                content: CONTINUE_AFTER_NON_ANSWER.to_string(),
                            });
                            force_next_tool_call = true;
                            continue;
                        }
                        // Give up nudging: return the best real content we have
                        // (usually a plan) instead of an empty non-answer.
                        let fallback = if content.trim().is_empty() {
                            best_effort_content.clone().unwrap_or(content)
                        } else {
                            content
                        };
                        let mut result = Self::parse_final_answer(
                            fallback,
                            reasoning,
                            plan,
                            trace,
                            changes,
                            draft_actions,
                        );
                        result.usage = usage_acc;
                        return Ok(result);
                    }

                    // Model tiers: if a strong synthesizer is configured and we
                    // actually gathered tool data, let it (re)write the final
                    // answer from that data — the cheap router drove the tool
                    // selection, the strong model writes the answer the user sees.
                    let final_content = match synthesizer.as_ref() {
                        Some(synth) if tool_calls_made => {
                            let (content, u) =
                                Self::synthesize_final(synth, &messages, tools, content).await;
                            usage_acc = usage_acc.saturating_add(u);
                            content
                        }
                        // Single-model main path: the final answer came from a
                        // mixed tool/answer turn, which can't carry response_format.
                        // If the model ATTEMPTED a structured JSON answer but botched
                        // it AND this provider can constrain output, re-emit through a
                        // dedicated final-answer turn (which applies response_format).
                        // Conditional on all three so the happy path and pure-prose
                        // answers pay no extra round-trip.
                        _ if tool_calls_made
                            && provider.supports_structured_output()
                            && parse_structured_final_answer(&content).is_none()
                            && looks_like_attempted_structured(&content) =>
                        {
                            let (content, u) =
                                Self::synthesize_final(&provider, &messages, tools, content).await;
                            usage_acc = usage_acc.saturating_add(u);
                            content
                        }
                        _ => content,
                    };
                    let mut result = Self::parse_final_answer(
                        final_content,
                        reasoning,
                        plan,
                        trace,
                        changes,
                        draft_actions,
                    );
                    result.usage = usage_acc;
                    return Ok(result);
                }
            }
        }

        Ok(ReasoningResult {
            content: "I analyzed your finances but ran out of reasoning steps. Here's what I found so far.".to_string(),
            reasoning: "The question was too complex for the iteration limit.".to_string(),
            plan,
            trace,
            changes,
            draft_actions,
            assumptions: Vec::new(),
            data_sources: Vec::new(),
            missing_data: Vec::new(),
            follow_up_questions: Vec::new(),
            response_blocks: Vec::new(),
            is_real_answer: false,
            hit_time_budget: false,
            usage: usage_acc,
        })
    }

    /// Have the strong synthesizer (re)write the final answer from the tool
    /// results already gathered. Falls back to the router's `fallback` answer on
    /// any miss (error, empty, or the model tries to call a tool anyway), so the
    /// tiering never makes the answer worse.
    async fn synthesize_final(
        synthesizer: &Arc<dyn CompletionProvider>,
        messages: &[ChatMessage],
        tools: &ToolSet,
        fallback: String,
    ) -> (String, TurnUsage) {
        let mut msgs = messages.to_vec();
        msgs.push(ChatMessage::User {
            content: FINAL_SYNTHESIS.to_string(),
        });
        match synthesizer
            .complete_final_answer_turn_with_usage(&msgs, &tools.definitions())
            .await
        {
            Ok((AssistantTurn::FinalAnswer { content, .. }, u)) if !content.trim().is_empty() => {
                (content, u)
            }
            // Synthesis missed (tool call / empty) but still cost tokens; keep
            // the fallback answer, count the usage.
            Ok((_, u)) => (fallback, u),
            Err(_) => (fallback, TurnUsage::default()),
        }
    }

    fn parse_final_answer(
        content: String,
        reasoning: String,
        plan: Vec<String>,
        trace: Vec<String>,
        changes: Vec<AgentChange>,
        draft_actions: Vec<crate::reasoning::messages::AgentDraftAction>,
    ) -> ReasoningResult {
        let Some(parsed) = parse_structured_final_answer(&content) else {
            // Not JSON, but may still be a genuine free-text answer — the
            // model sometimes answers a quick clarifying question or short
            // decline in plain prose instead of following the JSON contract.
            // Use the same test the run loop uses to decide whether a turn
            // needs a nudge, so a real answer here isn't downgraded to a
            // stall just because it skipped the JSON envelope.
            let is_real_answer = !content_after_plan(&content).is_empty();
            return ReasoningResult {
                content,
                reasoning,
                plan,
                trace,
                changes,
                draft_actions,
                assumptions: Vec::new(),
                data_sources: Vec::new(),
                missing_data: Vec::new(),
                follow_up_questions: Vec::new(),
                response_blocks: Vec::new(),
                is_real_answer,
                hit_time_budget: false,
                // Stamped by the caller from the run's accumulator.
                usage: TurnUsage::default(),
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
            plan,
            trace,
            changes,
            draft_actions,
            assumptions: parsed.assumptions,
            data_sources: parsed.data_sources,
            missing_data: parsed.missing_data,
            follow_up_questions: parsed.follow_up_questions,
            response_blocks: parsed.response_blocks,
            is_real_answer: true,
            hit_time_budget: false,
            // Stamped by the caller from the run's accumulator.
            usage: TurnUsage::default(),
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
             You are a general-purpose financial assistant: handle any personal-finance question the user asks — facts, balances, net worth, affordability, savings/emergency-fund timelines, spending and category/merchant analysis, income and cash flow, transaction search and date-range analysis, unusual/anomalous charges, recurring payments and subscriptions, budgeting and overspending prevention, and open-ended 'what should I do next' planning. The tools below are reusable capabilities; choose whichever ones fit the user's intent. These instructions are guidance for common intents, not an exhaustive script — generalize to new phrasings and new questions.\n\
             Always use tools before answering financial questions that depend on the user's data. Start with get_financial_snapshot for broad/open-ended questions unless a narrower deterministic tool clearly fits.\n\
             PLANNING: on your first turn, include a short plan as plain lines prefixed `PLAN:` followed by 3-5 numbered one-sentence steps, then a blank line. Emit this plan IN THE SAME TURN as your first tool call — the plan text and the tool call go together. Do NOT send the plan as a message by itself. Example first turn: the `PLAN:` block as content, plus a tool call to get_financial_snapshot.\n\
             PLAN:\n\
             1. Find the income that just landed\n\
             2. Rank every debt by interest rate\n\
             3. Recommend where each dollar should go\n\n\
             Emit the plan only once, on your first turn — never repeat it on later turns.\n\
             CRITICAL — never stall: every turn must EITHER include at least one tool call OR be your final JSON answer. Never end a turn with only prose (a plan, or a note that you will 'now' fetch or pull data) and no tool call — if you intend to use a tool, call it in that same turn. A turn that is just a plan or an announcement with no tool call and no final answer is a bug.\n\
             GROUNDING RULE: never invent, estimate, or guess a dollar figure, date, count, or percentage. Every number in your answer must come from a tool result or the provided context. If you do not have a number, say so and offer to look it up rather than fabricating one.\n\
             MONEY FORMAT: tool results give every amount both as an integer `X_cents` field AND a ready-formatted `X_display` string (e.g. \"-$2,200.00\"). When you state a dollar amount, copy the `_display` string — do NOT divide `_cents` by 100 yourself. If you must compute a new amount, do the math in whole cents and convert once at the end.\n\
             CLARIFY WHEN AMBIGUOUS: if the request is missing a detail you need (e.g. an amount for an affordability question, which goal/account/category, or which time range) or could reasonably mean different things, ask ONE concise clarifying question in follow_up_questions and give a brief answer explaining what you need — do not guess a specific number or pick arbitrarily and then compute on it. If a sensible default exists (e.g. 'this month', 'all accounts'), you may proceed but state the assumption.\n\
             FAIL GRACEFULLY: if a question needs data the user has not provided or a capability this local app does not have (e.g. live market prices, tax filing, external bank actions), say plainly what is missing or unsupported and suggest the closest thing you can do — do not fabricate an answer.\n\
             For unusual, suspicious, or anomalous transaction questions, call find_anomalies.\n\
             For recurring charges or subscription questions, call get_recurring_bills.\n\
             For income or cash-flow questions, call get_month_totals or run_cashflow_timeline.\n\
             For net worth or 'what am I worth' questions, call get_net_worth. Report accounts with an unknown/unconfirmed balance as UNKNOWN (never as $0), state how many are excluded, and note that the total omits them.\n\
             For 'where is my money going', biggest expenses, or overspending questions, call get_spending_breakdown (top categories, top merchants, and per-month totals) and then give concrete, behaviour-focused prevention steps (targeted budgets, subscription/recurring cuts, biggest-win categories).\n\
             For whether a specific one-time purchase is affordable (e.g. 'can I afford X for $N'), call run_purchase_affordability with the amount in cents; base the recommendation on emergency cash, monthly surplus, obligations, and high-interest debt, and be cautious.\n\
             For listing or analysing transactions by date range, amount threshold, merchant, or account (e.g. 'everything over $60 from Jan to June'), call search_transactions with the appropriate filters and report the returned date, merchant, amount, account, category, count, and total.\n\
             For paycheck or windfall allocation questions, call analyze_cash_inflow.\n\
             For goal timing questions, call calculate_goal_eta.\n\
             For debt ranking questions, call rank_debt_payoff; for payoff timelines or extra-payment comparisons, call run_debt_payoff_scenarios.\n\
             For multi-goal allocation questions, call run_goal_allocation_scenarios.\n\
             For emergency fund targets, 'when will my emergency fund be full', liquidity runway, or income-loss questions, call run_emergency_fund_scenarios; it defaults the savings rate to the current monthly surplus and returns an estimated completion date per target — report the target, current saved amount, monthly contribution used, and the completion date.\n\
             For savings-vs-debt tradeoff questions, call compare_debt_vs_goal.\n\
             For affordability, runway, monthly-surplus, or month-by-month balance questions, call run_cashflow_projection or run_cashflow_timeline.\n\
             For data sufficiency concerns, call get_data_quality_report.\n\
             EMPTY DATA: if the tools show no accounts and no transactions, tell the user plainly that no imported financial data is available (for example after a data reset) and that they should import data first. Do not fabricate balances, transactions, or summaries.\n\
             RECATEGORIZATION: to recategorize uncategorized transactions, first call list_uncategorized_transactions to get the rows and the available categories, then call draft_recategorization with one assignment per transaction (transaction_id + a category_id chosen from available_categories + a confidence). This only PREVIEWS the changes as a draft that the user must approve; nothing is written until they approve. In your answer, state how many were found, how many you proposed, and that the changes are awaiting approval. Never claim transactions were recategorized before approval.\n\
             When asked about investing, keep the answer principles-only; do not recommend tickers, ETFs, or market timing.\n\
             If emergency coverage is below one month or APR/minimum payment data is missing, say the answer is provisional and ask for the missing information.\n\
             When making a recommendation, compare at least two reasonable alternatives unless the answer is a simple fact lookup.\n\
             The final answer MUST be a JSON object with this schema: {{\"answer\":\"string\", \"reasoning\":\"string\", \"assumptions\":[\"string\"], \"data_sources\":[\"string\"], \"missing_data\":[\"string\"], \"follow_up_questions\":[\"string\"], \"response_blocks\":[...]}}.\n\
             The answer string may use concise Markdown for headings, bullets, tables, and code-style labels because the UI renders Streamdown markdown. Keep it readable while streaming.\n\
             Do not duplicate structured blocks in prose. Use response_blocks only when a visual block makes the answer clearer than prose alone; leave it empty for simple fact answers or short explanations.\n\
             Supported response_blocks are exactly: {{\"kind\":\"markdown\",\"markdown\":\"...\"}}, {{\"kind\":\"table\",\"title\":\"...\",\"columns\":[\"...\"],\"rows\":[[\"...\"]]}}, {{\"kind\":\"barChart\",\"title\":\"...\",\"seriesLabel\":\"...\",\"data\":[{{\"label\":\"...\",\"value\":123}}]}}, {{\"kind\":\"lineChart\",\"title\":\"...\",\"seriesLabel\":\"...\",\"data\":[{{\"label\":\"...\",\"value\":123}}]}}, {{\"kind\":\"metricGrid\",\"metrics\":[{{\"label\":\"...\",\"value\":\"...\",\"detail\":\"...\",\"tone\":\"neutral\"}}]}}, {{\"kind\":\"callout\",\"tone\":\"info\",\"title\":\"...\",\"body\":\"...\"}}, {{\"kind\":\"transactionTable\",\"count\":42,\"totalCents\":1193000,\"rows\":[{{\"date\":\"2026-05-03\",\"merchant\":\"...\",\"categoryKey\":\"...\",\"amountCents\":185000,\"flag\":null}}],\"more\":32}}, {{\"kind\":\"affordabilityVerdict\",\"canAfford\":true,\"headline\":\"Yes\",\"sub\":\"$540 · about 1% of liquid cash\",\"caveat\":\"Exceeds your May Shopping envelope by $426.\",\"fundingSource\":{{\"label\":\"Cover it from Travel\",\"detail\":\"$500 budgeted · $0 spent\"}}}}, {{\"kind\":\"categoryBreakdown\",\"periodLabel\":\"May\",\"rows\":[{{\"categoryKey\":\"Housing\",\"amountCents\":185000,\"isFixed\":true,\"isLever\":false}},{{\"categoryKey\":\"Dining\",\"amountCents\":41200,\"isFixed\":false,\"isLever\":true}}]}}, {{\"kind\":\"allocationSplit\",\"totalCents\":520000,\"segments\":[{{\"label\":\"Pay off Amex\",\"amountCents\":241800,\"rationale\":\"24.9% APR\",\"categoryKey\":\"debt\"}},{{\"label\":\"Emergency fund\",\"amountCents\":180000,\"rationale\":\"76% to target\",\"categoryKey\":\"savings\"}}]}}, {{\"kind\":\"rankedOptions\",\"title\":\"The three routes you asked about\",\"options\":[{{\"rankTone\":\"primary\",\"label\":\"Pay off the loan\",\"detail\":\"$2,418 → Amex Gold\",\"rationale\":\"Highest-interest debt at 24.9%.\"}},{{\"rankTone\":\"muted\",\"label\":\"Save for a car\",\"detail\":\"no active goal\",\"rationale\":\"Finish the emergency fund first.\"}}]}}, {{\"kind\":\"comparisonBars\",\"title\":\"Dining · this month vs average\",\"current\":{{\"label\":\"May 2026\",\"amountCents\":41200}},\"prior\":{{\"label\":\"12-mo avg\",\"amountCents\":36500}}}}, {{\"kind\":\"spendingReview\",\"months\":[{{\"period\":\"2026-05\",\"summary\":\"A steady month.\",\"actions\":[\"Glance at the PG&E bill\"]}}]}}, {{\"kind\":\"accountsOverview\"}}, {{\"kind\":\"spendTimeline\",\"title\":\"Monthly spend · Jan–Jul 2026\",\"subtitle\":\"last 3 months highlighted\",\"points\":[{{\"label\":\"Jan\",\"amountCents\":360000}},{{\"label\":\"Apr\",\"amountCents\":570000,\"annotation\":\"LISBON\"}},{{\"label\":\"Jul\",\"amountCents\":440000,\"highlight\":true,\"projected\":true}}]}}, {{\"kind\":\"spendingDrivers\",\"title\":\"What's driving the +$728/mo\",\"subtitle\":\"vs your Jan–Feb baseline\",\"drivers\":[{{\"label\":\"Travel\",\"tag\":\"planned\",\"amountDisplay\":\"+$213/mo\",\"note\":\"Italy flight deposits\"}}]}}, {{\"kind\":\"watchList\",\"title\":\"Watch out for these\",\"items\":[{{\"label\":\"The Amex balance\",\"detail\":\"$2,418 revolving at 24.9%\",\"amountDisplay\":\"−$50/mo\"}}]}}, {{\"kind\":\"actionPlan\",\"title\":\"Action plan\",\"items\":[\"Sweep the unused $168 into the House Fund\"]}}.\n\
             Use metricGrid for 2-6 headline numbers, table for alternatives/debt payoff/transaction review rows, barChart for category comparisons, lineChart for time trends, callout for missing-data/risk/next-action warnings, markdown only for a short supplemental section that should be visually separated, transactionTable specifically for search_transactions results (never the generic table kind for those), affordabilityVerdict specifically for a single purchase-affordability yes/no verdict typically produced from run_purchase_affordability results (never the generic callout or metricGrid kind for those), categoryBreakdown specifically for spending-by-category analysis typically produced from get_top_spending_categories or get_month_totals results, marking fixed-cost categories with isFixed and the single most controllable discretionary category with isLever (never the generic barChart or table kind for those), allocationSplit specifically for a paycheck/windfall allocation recommendation typically produced from analyze_cash_inflow results, splitting a total amount across labeled segments with a rationale for each (never the generic barChart or table kind for those; segments should sum to totalCents), rankedOptions specifically for comparing a small set of recommended courses of action against each other typically produced from rank_debt_payoff, compare_debt_vs_goal, or run_goal_allocation_scenarios results — use rankTone \"primary\" for the single best-ranked option, \"neutral\" for reasonable secondary options, and \"muted\" for options that should wait (never the generic table kind for those), and comparisonBars specifically for a single month-over-month or category-vs-average dollar comparison typically produced from get_month_totals or category spending results — current and prior amounts must be non-negative (never the generic barChart kind for a single two-point comparison). Use spendingReview for a multi-month spending review. This block is SERVER-RENDERED: for each month emit ONLY the period (\"YYYY-MM\"), a one-line summary, and 2-4 concrete action items — do NOT emit label, spentCents, categories, or subtitle; the app fills every number and category from the ledger for that period. get_spending_breakdown(months:N) already returns per-month category totals — do NOT call search_transactions per month. Use accountsOverview to list the user's accounts. This block is SERVER-RENDERED too: emit ONLY {{\"kind\":\"accountsOverview\"}} — no rows, balances, title, or badges; the app fills them from the ledger, so you never invent or omit a balance. Use spendTimeline for a month-by-month spend trend (2-24 points); set highlight true on the recent months you're focusing on, projected true on an incomplete current month, and annotation for a one-word cause of an outlier bar. Use spendingDrivers to break down what changed vs a baseline: one row per driver with a tag (planned/trend/prices/anomaly/creep/mixed), a signed per-month delta string in amountDisplay, and a short note — copy delta strings, do not compute them. Use watchList for a short ranked list of risks or things to watch (1-8 items), each a label plus a one-line detail and an optional cost string in amountDisplay. IMPORTANT: for any 'where am I spending more', 'what changed vs before', 'what's driving the increase', or 'what should I watch out for' question, you MUST emit spendTimeline (the monthly trend) and spendingDrivers (the per-driver breakdown), plus watchList when there are risks — NEVER present a monthly trend or a driver/what-changed breakdown as a prose Markdown table or as the generic table/barChart kind. A Markdown table of drivers in the answer text is wrong; put that breakdown in a spendingDrivers block instead. Use actionPlan for a short standalone checklist of concrete next steps (1-8 items) when the answer's value is the to-do list itself.\n\
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

/// Nudge sent when the model finalizes a turn without actually answering
/// (only a `PLAN:` preamble, or empty content, and no tool calls). Keeps the
/// loop from shipping a non-answer while gently reinforcing grounding.
const CONTINUE_AFTER_NON_ANSWER: &str = "You outlined a plan but have not answered yet. \
Now carry it out: call the tools you need to fetch the actual numbers from the user's data, \
then reply with ONLY the final JSON answer object. Do not restate the plan, do not leave the \
answer empty, and do not state any number you have not obtained from a tool result.";

/// Sent when the loop hits its wall-clock budget: force a final answer now from
/// what's already gathered, with no more tool calls.
const TIME_LIMIT_SYNTHESIS: &str = "You are out of time to gather more data. Using ONLY the tool \
results already in this conversation (and any CURRENT SNAPSHOT shown above), give your best, \
complete final answer NOW. Do not call any tools. If a detail is missing, answer with what you \
have and briefly note what you could not compute. Never state a number that is not in a tool \
result or the snapshot.";

/// Sent to the strong synthesizer when the fast router is done gathering, so it
/// writes the final answer from the tool results already in the conversation.
const FINAL_SYNTHESIS: &str = "You now have the data you need. Write your complete final answer in \
the required format, using ONLY the tool results above (and any CURRENT SNAPSHOT shown above). Do \
not call any tools, and do not state any number that is not in a tool result or the snapshot.";

/// Last-resort content when even the time-limited synthesis turn yields nothing:
/// summarize the steps taken so the user gets *something* actionable, never an
/// empty timeout.
fn summarize_progress(trace: &[String]) -> String {
    if trace.is_empty() {
        "I ran out of time before I could gather enough to answer. Please try a more specific \
         question."
            .to_string()
    } else {
        format!(
            "I ran out of time before finishing a full answer. Here's what I gathered so far:\n{}",
            trace
                .iter()
                .map(|t| format!("- {t}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// A compact, authoritative financial snapshot injected into the reasoning loop's
/// context so common planning questions can be answered with fewer tool
/// round-trips: the model may cite these pre-computed figures directly (the
/// GROUNDING RULE already permits "the provided context").
///
/// Every figure is sourced from the shared `finsight_core::metrics` layer using
/// the SAME two calls `context.rs` uses to build the Copilot's cashflow/balance
/// context — `balance_breakdown` and `rolling_averages(conn, 90)` — so the
/// snapshot is, by construction, the same number every screen (Today, Net Worth,
/// Wellness) shows and the same number the cashflow/balance tools return. It must
/// NOT hand-roll a savings rate, surplus, or liquid total on a different basis,
/// or the model would cite a figure that silently contradicts the app.
///
/// Returns an empty string if either metric fails, leaving the loop unchanged.
fn finance_snapshot_block(conn: &mut rusqlite::Connection) -> String {
    let Ok(bal) = finsight_core::metrics::balance_breakdown(conn) else {
        return String::new();
    };
    let Ok(roll) = finsight_core::metrics::rolling_averages(conn, 90) else {
        return String::new();
    };
    // Same EF definition as the Wellness screen (context.rs): EF-eligible balance
    // over the rolling 90-day average monthly expense.
    let ef_months = finsight_core::metrics::emergency_fund_months(
        bal.emergency_fund_cents,
        roll.avg_monthly_expense_cents,
    );
    let m = |c: i64| format!("${:.2}", c as f64 / 100.0);
    format!(
        "CURRENT SNAPSHOT (authoritative, pre-computed from the user's own data — these are the \
         SAME figures every screen shows, so you MAY cite them directly WITHOUT a tool call. Use \
         tools for category/merchant breakdowns, transaction history, per-liability debt detail, \
         goals, budgets, or anything not listed here):\n\
         - Liquid cash: {}\n\
         - Invested balance: {}\n\
         - Avg monthly income (rolling 90d): {}\n\
         - Avg monthly expenses (rolling 90d): {}\n\
         - Monthly net (income − expenses): {}\n\
         - Savings rate: {}%\n\
         - Emergency fund: {} ({:.1} months of expenses)\n\
         - Total debt owed: {}",
        m(bal.liquid_cents),
        m(bal.invested_cents),
        m(roll.avg_monthly_income_cents),
        m(roll.avg_monthly_expense_cents),
        m(roll.net_monthly_cents),
        roll.savings_rate_pct,
        m(bal.emergency_fund_cents),
        ef_months,
        m(bal.debt_cents),
    )
}

/// Returns the substantive content that follows any leading `PLAN:` preamble.
/// If the model emitted only a plan (or nothing at all), this is empty — the
/// signal that the turn is a non-answer that should not be finalized.
fn content_after_plan(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    let Some(plan_idx) = lines.iter().position(|l| l.trim() == "PLAN:") else {
        return raw.trim().to_string();
    };
    // Skip the contiguous run of numbered plan steps (and any blank separators)
    // that follow the `PLAN:` line; whatever remains is the real answer body.
    let mut i = plan_idx + 1;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.is_empty() {
            i += 1;
            continue;
        }
        match t.split_once(". ") {
            Some((num, _)) if num.trim().parse::<u32>().is_ok() => i += 1,
            _ => break,
        }
    }
    let rest = lines[i..].join("\n");
    // Drop trailing intent-filler like "Let me pull that data now." — the model
    // announcing it will act is not an answer; treat it as a non-answer so the
    // loop nudges the model to actually call tools rather than shipping intent.
    if is_intent_filler(&rest) {
        return String::new();
    }
    rest.trim().to_string()
}

/// True when text is only a short "I'll go do it now" announcement with no
/// substantive content — the model stating intent instead of answering.
fn is_intent_filler(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }
    // Only treat SHORT trailing text as filler; a real answer is longer.
    if t.len() > 120 {
        return false;
    }
    let lower = t.to_lowercase();

    // A question is directed at the USER, not an announcement of work. Asking
    // "which account did you mean?" is a correct answer to an ambiguous request,
    // and it opens with the same words as filler ("Let me know…"). Emptying it
    // here would drop `is_real_answer` and let `is_usable_tool_answer` discard
    // the turn in favour of the canned fallback — so the Copilot could never
    // cleanly ask for clarification or say what it still needs.
    if t.contains('?') {
        return false;
    }
    // Same for a stated information gap: "I'll need X before I can answer" is a
    // decline, not intent to act.
    //
    // "I'll need TO <verb>" splits both ways — "to know your retirement age" is
    // still asking the user, while "to fetch your accounts" is the model
    // announcing its own work. Only a data-gathering verb marks the latter, and
    // the list is deliberately the discriminator rather than a list of asking
    // verbs: an unrecognised verb then falls through as a real answer. Erring
    // toward keeping the reply ships an unhelpful sentence at worst, whereas
    // erring the other way resurrects the bug this whole guard exists to fix —
    // silently discarding a correct decline.
    const SELF_DIRECTED: [&str; 10] = [
        "fetch", "pull", "check", "look", "run", "gather", "retrieve", "query", "calculate",
        "compute",
    ];
    const NEEDS_FROM_USER: [&str; 4] = ["let me know", "i'll need", "i will need", "i'd need"];
    let asks_the_user = NEEDS_FROM_USER.iter().any(|p| {
        let Some(rest) = lower.strip_prefix(p) else {
            return false;
        };
        match rest.trim_start().strip_prefix("to ") {
            Some(after) => !SELF_DIRECTED
                .iter()
                .any(|v| after.trim_start().starts_with(v)),
            None => true,
        }
    });
    if asks_the_user {
        return false;
    }

    const INTENT_STARTS: [&str; 8] = [
        "let me",
        "i'll ",
        "i will ",
        "now i",
        "now let me",
        "let's ",
        "pulling ",
        "fetching ",
    ];
    INTENT_STARTS.iter().any(|p| lower.starts_with(p))
}

/// True when the model's final text looks like it was TRYING to emit the
/// structured JSON answer (starts with `{`, or mentions `response_blocks`) — as
/// opposed to a legitimate plain-prose reply. Combined with a failed
/// `parse_structured_final_answer`, this identifies a botched structured answer
/// worth re-emitting through a constrained turn, while leaving prose replies
/// (which need no extra round-trip) alone.
fn looks_like_attempted_structured(content: &str) -> bool {
    let t = content.trim();
    t.starts_with('{') || t.contains("\"response_blocks\"")
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
