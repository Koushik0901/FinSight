# FAQ

## Do I need a provider to use FinSight?

No. Budgets, goals, reports, transactions, and the deterministic parts of insights work without one. The Copilot and auto-categorization are the only features that need a provider — and both are opt-in. See [Configuring AI](/getting-started/configuring-ai).

## Does FinSight store my bank password?

No. For SimpleFIN, you exchange a SimpleFIN access URL obtained from your bridge. FinSight stores that URL inside your encrypted `data.sqlcipher` per user and uses it only when you synchronize. No bank password touches your server.

## Can I self-host without Docker?

The image is the supported distribution. Native `cargo run -p finsight-server` works for development (`FINSIGHT_DATA_DIR=./data`) but Docker + the hardened Compose file is the deployment path.

## What if I lose my recovery key and my password?

There is no way back — by design. Each database key is wrapped by both the password-derived KEK and the recovery key. Without either, the DB key cannot be unwrapped and the database cannot be opened. Back up the recovery key.

## How do I back up?

Two layers:

1. **Per-user snapshot** — Settings → Data & backups creates an encrypted snapshot under `users/<uuid>/backups/`.
2. **Disaster recovery** — back up the **whole `/data` volume** (`users.db` + `session.key` + every per-user database). A lone `data.sqlcipher` without `users.db` cannot be opened.

## Is my data synced across devices?

The server at `https://<your-origin>` is the source of truth. The browser’s seven-day IndexedDB cache is read-only and for offline viewing; it purges on logout. There is no sync to a vendor cloud.

## Which AI provider keeps data local?

[Ollama](/configuration/ollama) — inference stays on your LAN/machine. Cloud providers receive the redacted context for the task (merchant + amount for categorization; question-scoped FinancialContext for Copilot).

See also: [Privacy & Local Data](/getting-started/privacy), [Security & Privacy](/help/security).
