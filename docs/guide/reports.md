# Reports

Monthly trends, category and merchant tables, review snapshots, and exports.

## What Reports shows

- **Monthly trends** — income, expenses, and savings rate over the selectable window. Computed from `metrics`, same source as Today.
- **Category tables** — spend by category for the window, with prior-period and YTD comparisons.
- **Merchant tables** — top merchants by spend and frequency; merchant name uses the same normalization as Transactions.
- **Review snapshots** — canned review views (end-of-month, last 90 days) rendered in `reportWidgets`.

## Custom widgets

`repos/report_widgets` stores drag-ordered report widgets per user. The layout is persisted in your encrypted database; resetting it is a row delete, not a migration.

## Exports

- **CSV** — current Report window as CSV (streamed from the server; no third-party involved).
- **Per-user snapshots** — via **Settings → Data & backups**, which creates an encrypted snapshot under `users/<uuid>/backups/`.

## When numbers look wrong

Check the scope first:

| Symptom | Likely cause |
|---|---|
| One month spikes | Large transfer not linked as transfer — it counts as expense then income |
| Category total disagrees with Budget | Budget shows *budgeted* vs *spent*; Report shows *actuals* for the window |
| Net worth jumps | An account’s opening balance or a manual balance entry |

Ask the Copilot “Explain this report number” — the metric inspector shows definition, inputs, exclusions, and assumptions for that figure.

See also: [Budget](/guide/budget), [Transactions](/guide/transactions).
