# Overview — Using FinSight

FinSight is organized around a handful of screens you will visit in any session. This section walks through each one — what it shows, where its numbers come from, and what to do next.

## The daily loop

1. **Today** — check net worth, savings rate, and the one thing the Copilot thinks you should act on.
2. **Review** — clear the needs-review queue: categorize, merge transfers, split joint purchases.
3. **Budget / Goals** — nudge one envelope or make a contribution; see forecast update.
4. **Ask the Copilot** when you want a plan, not just a total.

## Map

| Screen | For | Key source |
|---|---|---|
| [Today](/guide/today) | Daily state | `finsight-core::metrics` + last close |
| [Accounts](/guide/accounts) | Holdings and history | `repos/accounts`, SimpleFIN sync runs |
| [Transactions](/guide/transactions) | Ledger detail | `repos/transactions` + transfer/split logic |
| [Budget](/guide/budget) | Envelope plan | `repos/budgets`, spending types |
| [Categories](/guide/categories) | Taxonomy & guidance | `repos/categories` + `spending/classify` |
| [Recurring](/guide/recurring) | Subscriptions & bills | `subscriptions` + `recurring` detectors |
| [Goals](/guide/goals) | Targets & compounding | `repos/goals` + projection at 7% |
| [Reports](/guide/reports) | Monthly trends & exports | `metrics`, `custom_report`, `report_widgets` |
| [Insights](/guide/insights) | Anomalies & patterns | `anomaly` + `agent_memory` |
| [Cash Flow](/guide/cashflow) | Safe-to-spend & calendar | `forecast` + `cashflow` |
| [Scenarios](/guide/scenarios) | What-if math | Deterministic finance tools via Copilot |
| [Recipes](/guide/recipes) | One-click fixes | Trusted recipe runner (`finsight-agent::recipe_runner`) |
| [Journey](/guide/journey) | Seven milestones | Wellness context + goal state |

Tip: amounts throughout the UI carry the `money` class so Privacy mode (eye icon) can blur them.

Next: [Today](/guide/today).
