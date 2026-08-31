# Scenarios

“What if I …?” forecasting with deterministic finance tools — natural-language entry, math exit.

## How it works

1. Type a question on **Scenarios** (e.g. “What if I raise rent to $2,400 and add $300/month to my emergency fund for a year?”).
2. The Copilot maps_free text to structured tool calls (adjust expenses, set contributions, change horizon).
3. `finsight-core::forecast` runs the math — balances forward from current ledger plus recurring assumptions — and returns a dated ledger projection.
4. The Copilot narrates the outcome with figures. The scenario is not saved unless you save it.

## Which numbers change

| Tool | Effect |
|---|---|
| `set_monthly_contribution` | Alters goal funding and end balance |
| `adjust_recurring` | Shifts cash-flow calendar and safe-to-spend |
| `set_expense` / `set_income` | Reprojects income/expenses and savings rate |
| `set_carryover` | Changes next-month opening envelope |

All tools are allow-listed in `planner.rs::ACTION_KINDS` and the system prompt forbids inventing totals.

## Saving & comparing

Save a scenario to revisit it after new transactions import — the projection refreshes against the latest ledger. Compare two saved scenarios side-by-side to see which trade-off is cheaper in 12 months.

See also: [Cash Flow](/guide/cashflow), [Copilot → Scenarios](/copilot/scenarios).
