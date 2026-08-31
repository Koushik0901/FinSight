# Accounts

Accounts hold your money and define net worth. FinSight supports manual accounts (you enter and import) and SimpleFIN-synced accounts (pulled from your bank via bridge).

## Types

`checking` · `savings` · `credit` · `investment` · `cash` · `loan` · `other`

Each carries a color token (e.g. `--c-checking: #60A5FA`) used in charts and indicators. Account creation lives in `crates/finsight-core::repos/accounts`.

## Manual accounts

1. **Accounts → Add account** — pick a type, name, and optional opening balance.
2. Import history via CSV or add transactions directly.
3. Balance history is derived from transactions; you never maintain a separate balance ledger.

## SimpleFIN accounts

1. Obtain a SimpleFIN access URL from your bridge.
2. Paste it during onboarding or at **Accounts → Connect SimpleFIN**. The URL is stored per-user inside your encrypted `data.sqlcipher`.
3. Press **Synchronize** — FinSight fetches accounts/transactions and merges them. Sync runs and errors are recorded; use them to diagnose a failing sync.

Removing the access URL stops future syncs; previously imported transactions stay.

## Net worth & balance history

Net worth = sum(asset accounts) − sum(liability accounts) at the chosen as-of date. Balance history is computed from transactions and shown as a sparkline on Today and a chart on Accounts.

## Multi-currency

FinSight does not invent exchange rates. Holdings in a foreign currency without a known rate are kept in an `unconverted` bucket and surfaced separately (see `CurrencyContext`). Totals you see are single-currency; unconverted holdings are listed, not silently summed.

## Management

- Edit an account name/type at **Accounts → Manage**.
- Assets/liabilities grouping — the Accounts header shows assets, liabilities, and net; the Drive with household members shows per-member share when enabled.
- Deleting an account removes its transactions only if you choose to; transfers referencing it become standalone rows.

See also: [Transactions](/guide/transactions), [Importing](/getting-started/importing-data).
