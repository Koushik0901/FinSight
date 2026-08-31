# Today

Today is the anchoring screen — one place that answers “Where do I stand?” and “What’s the one thing to do today?”

## What you see

- **Net worth + trend** — sum of asset accounts minus liabilities, with a sparkline from balance history. Derived from transactions and stored balances; no manual double-entry.
- **Income / Expenses / Savings rate** — this month’s totals and the savings-rate gauge. Pay Yourself First is healthy at ≥10%.
- **Category stream** — a thin allocation bar where each segment is a category’s share of spend. Source: this month’s categorized transactions.
- **Progress & More (collapsible)** — secondary metrics (emergency-fund months, snowball, allocation donut) live inside disclosure panels so the primary panel reads as one instrument.
- **Next action** — the Copilot’s highest-priority nudge (e.g. “Fund your emergency reserve before ambitious saving”) with a direct link.

## Privacy

All figures use the `money` class. Toggle the eye icon — numbers blur and tooltips hide amounts.

## Where numbers come from

`finsight-core::metrics` computes net/income/expenses and `wellness_context` computes emergency-fund months, snowball order, and allocation. The Today view reuses whatever `wellness_context` the Copilot would see, so totals never disagree between Today and the Copilot.

## What to do

- If savings rate < 10%, open [Budget](/guide/budget) and move one envelope toward Saving.
- If a subscription anomaly is flagged, check [Insights](/guide/insights).
- Ask the Copilot: “Why is my savings rate down this month?” — it cites the same metrics with inputs and exclusions.

See also: [Insights](/guide/insights), [Budget](/guide/budget), [Goals](/guide/goals).
