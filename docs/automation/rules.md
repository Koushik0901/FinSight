# Rules

Pattern rules and treatment rules — the “always do this” layer for incoming transactions.

## Types

- **Pattern rules** — `if merchant matches "SQ *BLUE BOTTLE" then category = Dining`. Stored under `repos/rules`, matched in `finsight-core::categorize`.
- **Treatment rules** — e.g. treat a payee as a transfer or a fee. Same engine, different effect.

Each rule shows a match count and an enable toggle. Disabled rules are kept, not deleted, so you can experiment safely.

## Creating a rule

1. Open **Rules** (or accept a Copilot-suggested rule from Insights).
2. Set the pattern (substring or normalized merchant), target category, and scope (any account vs specific).
3. Save — the rule runs on next categorization and on imports.

Rules are ordered; earlier rules win. Reorder when two patterns overlap.

## Agent categorization toggle

**Rules → Agent categorization** controls whether LLM categorization runs at all. When off, incoming rows are left uncategorized for rules + deterministic keyword heuristics. See [Categorization](/automation/categorization).

## Proposals

When the categorizer runs without a confident match, it may propose a rule. Review it in **Rules → Proposals** — accept, edit, or dismiss. A dismissed proposal will not recur for the same pattern.

See also: [Categorization](/automation/categorization), [Transactions](/guide/transactions).
