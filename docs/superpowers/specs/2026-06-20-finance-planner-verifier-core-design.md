# Finance Planner + Verifier Core Design

## Purpose

FinSight Copilot needs a reliable agent loop for personal-finance planning. The current app can answer several finance questions with deterministic tools and can run an LLM tool loop, but the orchestration is not yet rigorous enough for broad, nuanced planning. This sub-project builds the core planning and verification layer that future debt, goal, emergency-fund, and cashflow scenario engines will plug into.

The target behavior is a careful local-data financial analyst: gather relevant data, compare alternatives, expose assumptions, show missing data, avoid unsupported investment claims, and keep user-impacting changes as draft actions.

## Scope

This first implementation track covers the agent core, answer contract, and verification policy. It does not add broad new finance data models or a scenario-table UI.

In scope:

- A planner that classifies finance questions into supported task types.
- A required-tool plan before answering deep finance questions.
- A normalized structured answer shape for planning responses.
- A verifier that checks required sections, data completeness, math consistency, alternative comparisons, and investment guardrails.
- Integration with existing finance tools and deterministic calculators.
- Golden tests for supported workflows and verifier failure/downgrade cases.

Out of scope:

- Paycheck cadence, account liquidity labels, promo APRs, or other new persistence fields.
- Full standalone scenario engines for all finance domains.
- External market, tax, legal, interest-rate, or product research.
- New scenario-table UI.
- Autonomous financial mutations without user approval.

## Supported Workflows

The first version supports these planning task types:

- `cash_inflow_allocation`: paycheck, bonus, windfall, or similar one-time cash allocation.
- `goal_eta`: timeline for reaching a goal with a contribution amount and cadence.
- `debt_ranking`: snowball or avalanche ordering.
- `debt_vs_goal`: whether to preserve or draw down goal savings to pay debt.
- `financial_snapshot`: overall health, risks, and local-data summary.
- `investment_readiness`: whether investing is appropriate yet, principles-only.
- `general_finance_planning`: broader finance questions that require a tool plan and verifier-backed answer.

Unsupported or under-specified questions should ask for missing details instead of fabricating precision.

## Architecture

### FinancePlanner

`FinancePlanner` converts a user prompt into a `FinancePlan`.

The plan includes:

- `task_type`
- `required_tools`
- `optional_tools`
- `required_inputs`
- `missing_inputs`
- `planning_notes`
- `risk_flags`

The planner should be deterministic where possible. It can use existing string/profile inference for known workflows and can fall back to LLM classification only for ambiguous prompts. Tool selection must be explicit and testable.

Expected examples:

- Paycheck question with an amount: `analyze_cash_inflow`, `get_financial_snapshot`.
- Car-goal ETA with contribution/cadence: `get_financial_snapshot`, `calculate_goal_eta`.
- Car savings vs loan: `get_financial_snapshot`, `compare_debt_vs_goal`, and later `run_debt_payoff_scenarios`.
- Investment question: `get_financial_snapshot`; verifier enforces no ticker/ETF recommendations.

### Tool Executor

The executor runs the plan's required tools and returns typed `ToolEvidence`.

Each evidence item should include:

- `tool_name`
- `summary`
- `data_sources`
- `missing_data`
- `numbers_used`
- `raw_json`

Tool execution should remain read-only except for existing draft-action tools. Any write-like result must be represented as an approval-required draft action.

### FinanceAnswerBuilder

The builder turns the user prompt, plan, and tool evidence into a structured answer.

Answer fields:

- `recommendation`
- `summary`
- `alternatives`
- `numbers_used`
- `data_sources`
- `assumptions`
- `missing_data`
- `risks`
- `next_actions`
- `draft_actions`
- `confidence`
- `reasoning`

The plain prose shown in the UI can be derived from this structure, but the structured fields should be retained for validation, tests, and future UI improvements.

### FinanceVerifier

The verifier checks the structured answer before returning it.

Required checks:

- Required tools for the task type were run.
- Required answer sections are present for complex planning.
- Critical missing data is surfaced.
- Numeric claims are traceable to tool evidence or deterministic calculations.
- Debt and goal tradeoffs compare at least two alternatives.
- Investment answers are principles-only and contain no ticker, ETF, or market-timing advice.
- User-impacting changes are draft actions only.

Verifier output:

- `passed`
- `severity`: `ok | warning | blocked`
- `findings`
- `confidence_adjustment`
- `required_follow_up_questions`

If severity is `blocked`, Copilot should ask for missing data instead of returning a confident recommendation.

## Data Flow

1. User submits a Copilot question.
2. Command layer calls planner for deep finance questions.
3. Planner emits `FinancePlan`.
4. Tool executor runs required tools and captures `ToolEvidence`.
5. Answer builder creates a structured finance answer.
6. Verifier validates the answer.
7. Command layer maps the structured answer into the existing `AgentAnswer` response and stores traces/draft actions.
8. UI shows recommendation, reasoning, data used, missing data, alternatives, assumptions, and draft actions.

## Error Handling

- Missing required prompt inputs become follow-up questions.
- Missing local financial data becomes `missing_data`.
- Invalid tool arguments should produce a recoverable planner/verifier finding, not a panic.
- Tool failures should identify the failing tool and return a safe partial answer only when enough evidence remains.
- LLM JSON parsing failures should surface a clear provider error and should not corrupt stored action bundles.
- Verifier-blocked answers should avoid confident recommendations.

## Testing

Add focused tests at three layers.

Planner tests:

- Paycheck allocation selects `analyze_cash_inflow` and `get_financial_snapshot`.
- Goal ETA selects `calculate_goal_eta`.
- Debt vs goal selects `compare_debt_vs_goal`.
- Investment prompt selects snapshot and sets investment guardrail flags.
- Ambiguous prompt asks for required inputs.

Verifier tests:

- Blocks or downgrades answer when APR is missing for debt payoff planning.
- Blocks or downgrades answer when minimum payment is missing for payoff timing.
- Fails debt-vs-goal answer with no alternatives.
- Fails investment answer that names tickers or ETFs.
- Fails numeric claims that cannot be traced to evidence.
- Allows draft actions but rejects direct mutation claims.

Golden workflow tests:

- `$3,000 paycheck allocation`.
- `$500 biweekly car goal ETA`.
- `Should I use car savings to pay off a similar-sized loan?`
- Missing APR/minimum payment.
- Low emergency fund.
- Avalanche versus snowball ranking.

## Acceptance Criteria

- Deep finance questions go through planner, tool execution, answer builder, and verifier.
- Existing sample finance questions still pass with at least the same answer quality as the current app.
- Complex planning responses include recommendation, alternatives when relevant, numbers used, data sources, assumptions, missing data, next action, and confidence.
- Investment answers are principles-only and refuse ticker/ETF/market timing claims.
- User-impacting operations remain draft actions requiring approval.
- Tests cover planner selection, verifier guardrails, and golden workflows.
- `docs/agentic-finance-todo.md` is updated after implementation to reflect completed and remaining work.

## Implementation Notes

Prefer small modules inside `crates/finsight-agent/src/reasoning/` or a new `crates/finsight-agent/src/planning/` module:

- `planner.rs`
- `evidence.rs`
- `answer.rs`
- `verifier.rs`

Keep `crates/finsight-app/src/commands/agent.rs` thin. It should orchestrate app state, call the agent crate, and map results to Tauri response types. Avoid growing the command file with more finance business logic.

Existing deterministic finance functions in `crates/finsight-agent/src/finance.rs` should remain reusable; do not duplicate scenario math in prompt strings.
