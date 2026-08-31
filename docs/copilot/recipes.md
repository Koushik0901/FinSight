# Recipes

Recipes are trusted, multi-step automations the Copilot can propose and you approve — short, readable, and reversible.

Examples:

- “Create next month’s envelopes from last month’s actuals.”
- “Move $400/month to emergency fund for 6 months.”
- “Roll over unspent groceries to Savings.”

## How recipes work

1. **Discover** — open **Recipes** or accept a Copilot suggestion. The runner (`finsight-agent::recipe_runner`) lists steps, expected effects, and required confirmations.
2. **Review** — every tool call and argument is shown before execution via the same RPC dispatcher as any other action.
3. **Run** — confirm; the runner executes the sequence and reports per-step results.
4. **Review again** — post-execution a card appears with status and undo hints where applicable.

## Recipes vs rules

|  | Recipes | Rules |
|---|---|---|
| Trigger | You do | Incoming rows do |
| Scope | Multi-step, cross-cutting | Single pattern → category/treatment |
| Confirmation | Per-recipe | Per-rule enable/disable |

Recipes are **not** silent — no step runs without your confirmation. Remove or edit a generated envelope, contribution, or transaction like any other ledger row.

See also: [Guide → Recipes](/guide/recipes), [Rules](/automation/rules).
