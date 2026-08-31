# Goals

Targets you can contribute to, with a built-in compound-growth projection.

## Creating a goal

1. **Goals → New goal** — name, target amount, optional deadline.
2. Pick a funding account (contributions are transactions into the goal bucket).
3. The card shows contributed / target, remaining, and a thin progress bar. Emergency-fund goals offer a quick-fill button tied to `wellness_context.emergency_fund_months`.

## Contributions

Record a transaction categorized to the goal’s bucket or use **Contribute** on the goal card — both are the same ledger rows with a goal linkage (`repos/goals`). History is shown on the card.

## Projections

The what-if projection computes 10/20/30-year wealth from the current contribution rate at **7% annual** (Hill / Kiyosaki) and renders it on the goal. It is deterministic arithmetic, not a prediction — the Copilot is instructed to say so. Change the monthly amount to see the long-range effect instantly.

## Types

| Goal | How FinSight treats it |
|---|---|
| Emergency fund | Quick-fill offered; months-of-cover gauge derived from expenses |
| Debt payoff | Ordered smallest-balance-first for the snowball; see also [Journey](/guide/journey) |
| Saving / Investment | Long-range projection rendered; conscious-spending type is Saving or Investment |

## What-if

The goal screen’s what-if grid lets you compare “what if I add $200/month for 12 months?” across horizons without writing a scenario. For richer “what if I move house / change jobs” questions, use [Scenarios](/guide/scenarios).

See also: [Budget](/guide/budget), [Financial Framework](/framework/overview).
