# Agentic Finance Capability Backlog

This document tracks the work needed for FinSight Copilot to become a robust personal-finance planning agent with strong reasoning depth, tool orchestration, and nuanced local-data analysis.

## Tracked on GitHub

This file stays the source of truth for full detail and current status. The
distinct, pickable items also have issues so they can be scheduled, linked to a
PR, and seen without opening this file (issue #12):

| Item | Issue |
|---|---|
| Conversational categorization of uncategorized transactions | [#28](https://github.com/Koushik0901/FinSight/issues/28) |
| Agent-triggered navigation after an approved action | [#29](https://github.com/Koushik0901/FinSight/issues/29) |
| Goal priority and deadline strictness | [#30](https://github.com/Koushik0901/FinSight/issues/30) |
| Confidence scoring for recurring detection | [#31](https://github.com/Koushik0901/FinSight/issues/31) |
| User risk tolerance / financial philosophy | [#32](https://github.com/Koushik0901/FinSight/issues/32) |
| Hybrid and custom debt-payoff strategies | [#33](https://github.com/Koushik0901/FinSight/issues/33) |
| Promotional APR expiry modeling | [#34](https://github.com/Koushik0901/FinSight/issues/34) |
| Dedicated sinking-fund planning | [#35](https://github.com/Koushik0901/FinSight/issues/35) |
| Actionable missing-data prompts | [#36](https://github.com/Koushik0901/FinSight/issues/36) |

When an item here becomes concrete enough to pick up, open an issue and add it
to this table — a `[ ]` that exists only in this file is one nobody schedules.

## Status Legend

- `[x]` Implemented in the current app.
- `[~]` Partially implemented; useful, but not complete enough for the target standard.
- `[ ]` Planned work.

## Target Standard

Copilot should behave like a careful financial analyst operating only on the user's local FinSight data. It should gather the right data, compare alternatives, expose assumptions, ask for missing inputs, avoid unsupported investment claims, and produce actionable plans that remain explainable and user-approved.

## Current Capability

As of the current implementation, Copilot has a finance vertical slice that can answer several common planning questions from local FinSight data:

- `[x]` Paycheck or windfall allocation. It can split an amount across starter emergency fund, high-interest debt, goal savings, and investing readiness.
- `[x]` Goal ETA. It can estimate when a user reaches a goal from weekly, biweekly, semimonthly, or monthly contributions.
- `[x]` Debt ranking. It can rank debts by avalanche or snowball using liability balance, APR, and minimum payment data.
- `[x]` Debt versus goal tradeoff. It can compare car savings against a loan or credit card, protect an emergency-fund floor, model payoff months, estimate interest impact, and show alternatives.
- `[x]` Local finance snapshot. It can inspect liquid balances, total balances, 90-day and 12-month income/expense averages, goals, liabilities, recurring bills, planned transactions, and data warnings.
- `[x]` Provider JSON robustness. Anthropic, OpenAI-compatible, and Ollama providers now accept JSON arrays and objects and tolerate text around JSON.
- `[x]` Tool visibility. Copilot displays tool use and local data sources used by a response.
- `[x]` Missing-data visibility. Copilot surfaces missing APRs, missing minimum payments, uncategorized transactions, and other data quality warnings when relevant.
- `[x]` Investment guardrails. Investing advice is readiness/principles-only and does not recommend tickers, ETFs, or market timing.
- `[x]` Draft-action safety. Agent action tools create draft actions for user approval rather than mutating data autonomously.
- `[x]` Golden tests exist for the core sample questions: paycheck allocation, biweekly car-goal ETA, and car savings versus similar-sized loan.
- `[x]` Dedicated deterministic scenario tools now exist for debt payoff timelines, goal allocation, emergency-fund targets, cashflow timelines, large-purchase affordability, goal/bill conflict checks, and data-quality reporting.

Important current limits:

- `[~]` The visible Copilot now tries the LLM tool-calling loop first for deep finance questions, accepts it only when it returns tool-backed structured answer metadata, then falls back to the deterministic verified planner for supported workflows. Broad unsupported questions can still return provisional tool-loop answers when the final schema is incomplete.
- `[~]` The reasoning engine can call tools over multiple turns and now asks for a structured final JSON object carrying assumptions, data sources, missing data, follow-ups, and reasoning. The app validates that deep answers used tools and produced data sources before accepting them; deeper field-level schema validation for all broad LLM answers is still planned.
- `[~]` Dedicated deterministic engines now exist for debt payoff scenarios, goal allocation, emergency-fund targets, cashflow timelines, large-purchase affordability, goal/bill conflict checks, and data quality. They consume account liquidity/earmark metadata, savings APY, loan original balance/start date, and optional paycheck cadence settings where relevant, but still need richer due dates, promo APRs, statement balances, exact purchase carrying costs, and full income schedules.
- `[~]` Data-source citations are section-level sources, not row-level citations for every number.
- `[~]` Missing data is shown, and supported debt payoff/debt-vs-goal workflows now block high-confidence recommendations when critical APR or minimum-payment data is absent. Broader unsupported LLM answers still need full critical-data enforcement.
- `[ ]` The app still does not use external market, tax, legal, interest-rate, or product data; this is intentional for the current local-data-only version.

## Core Agent Architecture

- `[x]` Make the tool-calling reasoning engine the default path for deep finance questions.
- `[x]` Keep deterministic calculators available as tools.
- `[x]` Add a planner step that selects required tools before answering for supported finance workflows.
- `[~]` Add a verifier step that checks required tools, data sufficiency, investment guardrails, and whether alternatives were compared. It now blocks debt payoff/debt-vs-goal recommendations when critical APR or minimum-payment data is missing; numeric trace checks are still basic.
- `[~]` Add answer schemas for recommendations, scenario comparisons, assumptions, missing data, citations, and draft actions. Structured planner answers exist, command responses now expose scenario alternatives for planner/direct supported workflows, and tool-loop final answers carry structured assumptions/data-sources/missing-data/follow-ups; citations still need row-level structure.
- `[ ]` Persist conversation context and prior user preferences in a controlled memory layer.
- `[x]` Add replayable traces for financial recommendations through visible tool trace and saved bundles.
- `[x]` Add confidence scoring based on data completeness, not only model confidence.

## Finance Scenario Engines

- Debt payoff engine:
  - `[x]` Avalanche and snowball ranking.
  - `[x]` Amortization with APR, minimum payments, and extra/redirection payments for active debts with complete APR/minimum-payment data.
  - `[ ]` Hybrid and custom priority strategies.
  - `[ ]` Promo APR expiry, payment due dates, and statement balance modeling.
  - `[~]` Interest saved and months saved for debt-vs-goal scenarios.
  - `[ ]` Credit-utilization impact.

- Savings and goals engine:
  - `[x]` Goal ETA by weekly, biweekly, semimonthly, and monthly contributions.
  - `[~]` Semimonthly and irregular contributions. Semimonthly is implemented for goal ETA; irregular contribution schedules remain planned.
  - `[~]` Multi-goal allocation with priority, deadlines, current balances, and required monthly savings. Dedicated tool exists; goal priority/deadline strictness data model is still basic.
  - `[~]` Basic sinking-fund support through existing goals.
  - `[ ]` Dedicated sinking-fund planning for car, travel, insurance, repairs, taxes, and annual expenses.

- Emergency fund engine:
  - `[~]` Starter and one-month target logic.
  - `[x]` Three-month and six-month target planning.
  - `[~]` Liquidity floor using liquid account balance.
  - `[~]` Impact of using earmarked savings for debt.
  - `[~]` Runway under income loss or reduced income. Income-loss runway exists; reduced-income what-if depth is still basic.

- Cashflow timeline engine:
  - `[~]` Upcoming bills and planned transactions are available in the snapshot.
  - `[~]` Paycheck cadence modeling through optional planning settings; exact income schedule records are still missing.
  - `[~]` End-of-month balance forecast from average income/expenses and planned transactions; exact bill/paycheck timing still needs cadence data.
  - `[~]` Low-balance warnings exist; transfer recommendations still need liquidity/earmark metadata.
  - `[~]` Basic what-if cashflow projection tool, plus one-time large-purchase affordability modeling and goal-contribution conflict checks against upcoming obligations, emergency cash, surplus, and high-interest debt.

- Investment readiness engine:
  - `[x]` Principles-only advice from local data.
  - `[x]` Readiness gates: emergency fund and high-interest debt.
  - `[x]` Readiness gates: cashflow stability and goal deadlines.
  - `[x]` No ticker, ETF, market timing, tax, or legal recommendations without explicit external data support.

## Tooling

- `[x]` Add `run_debt_payoff_scenarios`.
- `[x]` Add `run_goal_allocation_scenarios`.
- `[x]` Add `run_emergency_fund_scenarios`.
- `[x]` Add `run_cashflow_timeline`.
- `[x]` Add `run_purchase_affordability`.
- `[x]` Add `run_goal_conflict_scenario`.
- `[x]` Add `get_data_quality_report`.
- `[~]` Add `explain_recommendation_sources` via current `dataSources` field.
- `[x]` Add `draft_budget_changes` through existing draft action patterns.
- `[x]` Add `draft_goal_contribution_changes`.
- `[x]` Add `draft_debt_payment_plan`.
- `[ ]` Add transaction categorization tools for Copilot:
  - List uncategorized transactions with enough context for review: transaction id, posted date, merchant, amount, account, existing notes, inferred merchant key, and candidate categories.
  - Accept natural-language categorization rules from the user, such as "Amazon under $50 is shopping, paychecks are income, Shell is transport."
  - Apply rules as draft actions first, then let the user approve all or selected category updates.
  - After approval, refresh affected transaction/category/budget/report queries so graphs, plots, and summaries reflect the new categories without requiring manual navigation or app restart.
  - Consider saving high-confidence repeated mappings as rule proposals or agent memory rather than silently creating permanent rules.
- `[~]` Make tools return structured citations and data warnings.
- `[~]` Require tool outputs to include units, dates, account scope, and data freshness.
- `[x]` Add strict tool argument validation and friendly recovery from invalid tool calls.
- `[~]` Support multi-tool plans where one tool's output feeds another. Reasoning prompt and toolset now expose dedicated scenario tools; deterministic planner chaining is still limited.

## Data Model Improvements

- `[~]` Add paycheck cadence and expected income schedules. Optional paycheck cadence/amount settings exist; full recurring income schedule records remain planned.
- `[~]` Add liability payment history, due dates, statement balances, promo APRs, and credit limits. Current model has balance, APR, minimum payment, credit limit, payoff date, original balance, and start month.
- `[x]` Add account liquidity type, emergency-fund eligibility, and goal earmark metadata.
- `[x]` Add savings APY metadata for savings accounts and expose it to Copilot context.
- `[ ]` Add goal priority, deadline strictness, and whether goal balances are spendable for other needs.
- `[ ]` Add recurring income and bill confidence scores.
- `[ ]` Add user risk tolerance and financial philosophy preferences.
- `[ ]` Add explicit tax jurisdiction only if future versions use tax-aware planning.

## Answer Quality and Safety

- `[~]` Every planning answer should include a recommendation.
- `[~]` Every planning answer should include alternatives compared.
- `[~]` Every planning answer should include numbers used.
- `[x]` Every planning answer should include data sources.
- `[x]` Every planning answer should include missing data.
- `[x]` Every planning answer should include assumptions.
- `[~]` Every planning answer should include next action.
- `[x]` Every planning answer should include what would change the recommendation for structured planner answers.

- `[x]` Refuse or narrow specific stock/ETF/ticker recommendations.
- `[x]` Refuse or narrow market timing.
- `[~]` Refuse or narrow legal/tax claims without supported data.
- `[~]` Refuse or narrow recommendations that require missing APR, minimum payment, income, or liquidity data. Supported debt payoff/debt-vs-goal workflows now block on missing APR/minimum payment; investment readiness now gates on unstable cashflow and near-term goal deadlines; broader income/liquidity enforcement remains partial.

- `[x]` Add a verifier that checks answer sections for supported complex planning workflows.
- `[~]` Add deterministic math checks for calculated figures before displaying them.
- `[~]` Ask clarifying questions when a confident answer would be unsafe.

## Evaluation and Testing

- `[x]` Golden test: paycheck allocation across emergency fund, credit card, loan, car goal, and investing readiness.
- `[x]` Golden test: biweekly and semimonthly savings ETA for a car goal.
- `[x]` Golden test: car savings versus similar-sized loan with interest saved and payoff timeline changes.
- `[x]` Golden test: missing APR/minimum payment blocks payoff timeline confidence.
- `[~]` Golden test: low emergency fund.
- `[x]` Golden test: multiple debts with avalanche vs snowball.
- `[x]` Golden test: affordability of a large purchase.
- `[x]` Golden test: goal conflict with upcoming recurring bills.
- `[x]` Add fixture databases with representative household profiles.
- `[x]` Add snapshot tests for structured answer fields.
- `[x]` Add tool orchestration tests that verify required tools for supported workflows.
- `[x]` Add provider contract tests for JSON arrays and objects.
- `[x]` Add provider contract tests for malformed model output recovery.
- `[x]` Add regression tests for investment guardrails.

## Product UX

- `[x]` Show data sources and tool trace by default for complex answers.
- `[x]` Show scenario tables for alternatives.
- `[x]` Let users approve draft actions individually.
- `[~]` Let users save a scenario and revisit assumptions later.
- `[ ]` Make missing data actionable with links to edit liabilities, goals, accounts, or planned transactions.
- `[~]` Add a "why this recommendation" explainer through reasoning text and data sources.
- `[ ]` Let Copilot navigate the app after user-approved actions or when a screen is the clearest way to inspect a result.
  - Support route targets such as Today, Transactions, Budget, Reports, Settings, Goals, Rules, and Categories.
  - Prefer navigation as a follow-up action after Copilot changes or drafts data, for example "I updated 18 uncategorized transactions. Open Reports to review the updated spending breakdown?"
  - Allow explicit user requests such as "take me to the budget page" while keeping this secondary to finance workflows.
  - Expose navigation as a UI-level command/event from Copilot rather than as a backend finance tool, because it changes app state, not financial data.
  - Use this to show the result of a change directly, such as opening Transactions filtered to newly categorized rows or Reports after category updates.

## Milestones

1. Tool-first finance vertical slice:
   - `[x]` Provider JSON fixes.
   - `[x]` Deterministic calculators exposed as tools.
   - `[x]` Scenario comparison for paycheck, goal ETA, and debt-vs-goal.
   - `[x]` Data sources, missing data, and investment guardrails.
   - `[x]` Golden tests for core questions.

2. Scenario engine expansion:
   - `[x]` Dedicated debt, goals, emergency fund, and cashflow timeline tools.
   - `[~]` Better data model support for due dates, paycheck cadence, and liquidity. Liquidity and basic paycheck settings are implemented; due dates and full income schedules remain.

3. Agent planner and verifier:
   - `[x]` Explicit plan-before-answer loop for supported finance workflows.
   - `[~]` Answer schema validation through `FinanceVerifier`, including required recommendation-change conditions for structured planner answers.
   - `[~]` Tool-call recovery and structured final-answer extraction are implemented; deeper math verification remains partial.

4. Evaluation suite:
   - `[x]` Fixture households.
   - `[x]` Golden answer snapshots.
   - `[x]` Provider/tool contract tests, including malformed model output rejection, recoverable invalid tool calls, and structured tool-loop final answers.

5. UX for trust:
   - `[x]` Scenario comparison UI.
   - `[ ]` Actionable missing-data prompts.
   - `[~]` Approval workflow polish.

6. Copilot operator UX:
   - `[ ]` Conversational uncategorized-transaction review and category assignment.
   - `[ ]` Agent-triggered app navigation for showing the result of approved changes.





