# Recipes

Trusted, one-click automations for budgets, cleanup, goals, and reviews.

## What recipes are

Recipes are curated action bundles in `finsight-agent::recipe_runner` — short, reversible sequences the Copilot can propose and you approve. Examples:

- “Roll over unspent groceries to Savings.”
- “Create envelopes from last month’s actuals.”
- “Move $400/month to emergency fund for 6 months.”

Each recipe lists its steps, expected effects, and required confirmations before it runs.

## Running a recipe

1. Open **Recipes** or accept a Copilot-suggested recipe.
2. Read the plan — every tool call and its arguments are shown.
3. Confirm. The runner executes the sequence via the same RPC dispatcher as every other action and reports per-step results.
4. Review the produced transactions/adjustments in the ledger.

Recipes are **trusted** but not silent — no step runs without your confirmation, and post-execution a review card appears in the Copilot with undo hints where applicable.

## Recipes vs Rules

|  | Recipes | Rules |
|---|---|---|
| When | One-off, you trigger it | Ongoing, matches incoming rows |
| Scope | Multi-step, cross-cutting | Single pattern → category/treatment |
| Review | Shown as a plan, confirm once | Enabled/disabled, no per-row confirm |

See also: [Rules](/automation/rules), [Copilot → Recipes](/copilot/recipes).
