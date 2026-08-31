# Importing Your Data

FinSight imports from CSV and from SimpleFIN. Both paths keep your server as the writer — nothing is pushed to a cloud ledger.

## CSV import

Supported parsers live in `crates/finsight-providers`. Supported formats include common bank/credit CSV exports with columns for date, description, and amount.

1. Go to **Accounts** and open the CSV import drawer, or use the onboarding importer.
2. Upload one or more CSV files. FinSight parses them in `crates/finsight-providers` and stages `import_candidates`.
3. Review the candidate list: merchant normalization, duplicate detection (same date/amount/description within tolerance), and fee handling are applied.
4. Confirm to finalize — staged rows become transactions. Re-importing the same file is deduplicated.

Files are staged under `users/<uuid>/imports/` (authenticated, per-user). You can export your data again from **Settings → Data**.

### Troubleshooting CSV

- **Wrong amount sign?** Some banks export credits as negative — flip the sign mapping in the import preview.
- **Duplicate transactions?** FinSight skips exact duplicates within a short window; adjust the account if you see over-merging.
- **Unconverted currencies?** Holdings in a foreign currency without a known rate are surfaced separately rather than invented.

## SimpleFIN

SimpleFIN Bridge connects your bank to FinSight without storing your bank password on your server — you exchange a SimpleFIN access URL.

1. Obtain a SimpleFIN access URL from your bridge.
2. Paste it at **Onboarding** or **Accounts → Connect SimpleFIN**.
3. FinSight stores the URL inside your encrypted `data.sqlcipher`, per user, and uses it only when you press **Synchronize**.
4. Sync runs fetch accounts and transactions and merge them; the sync run history shows what was pulled.

You can remove the access URL at any time — synchronization stops, existing imported transactions remain.

## After import

- **Review queue** — uncategorized or low-confidence transactions land in **Insights → Needs Review** and **Transactions → Review**.
- **Categorization** — an LLM provider (if configured) proposes categories; deterministic rules and keyword fallbacks apply otherwise. See [Categorization](/automation/categorization).
- **Balance history & net worth** — derived by `finsight-core` from transactions and stored balances; no manual double-entry.

Next: [Configuring AI](/getting-started/configuring-ai).
