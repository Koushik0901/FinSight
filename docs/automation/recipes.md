# Recipes — Automation

Recipes are the one-off, multi-step companion to rules.

|  | Recipes | Rules |
|---|---|---|
| When | You trigger it | Incoming rows trigger it |
| Scope | Multi-step, cross-cutting | Single pattern → category/treatment |
| Confirmation | Per-recipe plan | Per-rule enable/disable |

## Examples

- “Roll over unspent groceries to Savings.”
- “Create envelopes from last month’s actuals.”
- “Move $400/month to emergency fund for 6 months.”

## Running

1. Open **Recipes** or accept a Copilot-suggested recipe.
2. Read the plan — every tool call and argument is shown before execution.
3. Confirm. The runner (`finsight-agent::recipe_runner`) executes via the same RPC dispatcher as any other action and reports per-step results.

No step runs without your confirmation. Results remain ordinary ledger rows you can edit or revert.

See also: [Guide → Recipes](/guide/recipes), [Copilot → Recipes](/copilot/recipes).
