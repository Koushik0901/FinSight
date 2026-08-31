# Financial Framework — Overview

FinSight’s guidance is shaped by six durable ideas, not motivational slogans. Each becomes a calculation the Copilot can cite.

| Principle | Source | Where it lives in FinSight |
|---|---|---|
| Pay Yourself First (≥10%) | Babylon / Ramsey | Savings rate on Today; Babylon nudge; Copilot priority #1 |
| Emergency Fund (3–6 months) | Ramsey / Sethi | Quick-fill on Goals; `wellness_context.emergency_fund_months` |
| Debt Snowball (smallest first) | Ramsey | `wellness_context.debt_snowball` ordered by balance ASC |
| Conscious Spending (Need/Want/Saving/Investment) | Sethi | `spending_type` on categories; allocation donut on Budget |
| Compound Growth (7% annual) | Hill / Kiyosaki | 10/20/30-year projection on Goals |
| Behaviour over math | Housel | Nudges surface patterns, not just totals |
| Financial Journey | All | `/journey` — seven milestones from stability to freedom |

## How the framework reaches the user

1. **Metrics** (`finsight-core::metrics` + `wellness_context`) compute emergency-fund months, snowball order, allocation, savings rate.
2. **Prompt** (`planner.rs::build_system_prompt`) embeds the framework so every LLM call is steered the same way.
3. **Screens** (Today, Budget, Goals, Journey) render the same metrics with inputs and caveats exposed in the inspector.

No screen invents advice. The Copilot’s job is to translate the metric into a next step: fund the emergency buffer before ambitious saving, pay the smallest debt first for momentum, keep wants from quietly eating savings.

Next: [Pay Yourself First](/framework/pay-yourself-first).
