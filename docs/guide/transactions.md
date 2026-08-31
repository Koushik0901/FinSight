# Transactions

The ledger. Every screen ultimately reads from here — Today, Budget, Reports, and the Copilot all compute from the same rows.

## Finding transactions

- **Search** — merchant substring, amount, or note. Merchant is normalized (`finsight-core::merchant`) so “SQ *BLUE BOTTLE” and “blue bottle” group together.
- **Filters** — date range, account, category, transfer/split, and review state. Saved filters live in the TransactionFilter drawer.
- **Review queues** — uncategorized, needs-review, possible transfers, and possible splits are prefiltered for you. See [Insights](/guide/insights).

## Editing

Open any row → the **Transaction drawer**:

- Change date, amount, merchant, category, account, or notes.
- **Transfers** — link two transactions (e.g. checking → savings) into a single transfer. The double-entry is stored as a pair with a shared identity; either side can be edited and both update.
- **Splits** — break one purchase (e.g. groceries + household) into categorized children. The parent amount must equal the sum of children; the drawer enforces this.
- **Unconverted currencies** — a banner appears when a transaction’s account currency differs and no rate exists; the amount is kept faithfully, not converted.

## Transfers vs duplicates

| Situation | Do |
|---|---|
| Same amount leaving account A and arriving in B on the same day | Link as transfer |
| Two identical CSV rows from re-import | Dedup guard skips it |
| Same payee, different amounts, consecutive days | Leave as two transactions |

## Merchant & categories

Merchant raw text is kept, a cleaned `merchant_key` is indexed, and a centroid embedding may be stored for semantic categorization. See [Categorization](/automation/categorization).

## Performance

The transaction list is virtualized and query-cached (tanstack-query + seven-day IndexedDB persist). Filters that touch merchant text use indexed lookups.

See also: [Categories](/guide/categories), [Rules](/automation/rules).
