# Categories

Categories are the taxonomy the ledger speaks in. Everything — Today, Budget envelopes, Reports, and the Copilot — aggregates by category.

## Structure

- **Category** — name, color swatch, and group. Groups are user-defined (e.g. “Living”, “Growth”).
- **Spending type** — `Need` · `Want` · `Saving` · `Investment` (Sethi). Powers the allocation donut on Budget and the wellness context. Set it per category; the Copilot treats miscoded types as a fixable problem.
- **Guidance** — optional per-category hint rendered in the picker (e.g. “Groceries · Need · 12% of income last month”).

## Views

**Categories → Month / Prior period / YTD.** Each scope shows spent, budgeted, and count. Change the scope to catch drift: a category that looks calm this month may be 40% over YTD.

## Guidance & spending type

The category drawer shows the spending type and the last few months of spend so you can judge a type change. Moving “Dining” from Want to Need shifts the allocation and changes the Copilot’s nudges — it will warn differently.

## Palettes

Swatches come from `palette.rs`. The category stream bar and donor bars reuse those swatches so a color means the same thing in Today, Budget, and Reports.

## When to edit vs create

| Need | Do |
|---|---|
| Groceries vs Dining blur together | Keep two categories; fix with rules |
| One-off tax payment | Create a “Taxes” category, or use an existing “Government” if you have one |
| Category feels like two things | Split the transaction instead of inventing a hybrid category |

See also: [Transactions](/guide/transactions), [Rules](/automation/rules).
