# SimpleFin Phase 4 Advanced Sync Design

**Date:** 2026-06-28
**Status:** Approved
**Scope:** Full Phase 4 — batch sync, background sync, transfer detection, holdings/securities, balance-drift alerts.

## Context

FinSight SimpleFin integration currently supports per-account manual sync, per-connection credential storage, and persisted institution/connection metadata (Phase 3). Phase 4 adds automation, richer investment-account support, and data-quality monitoring.

## Source-of-truth findings

The SimpleFIN protocol specification (v2.0.0-draft, https://www.simplefin.org/protocol.html) defines four endpoints (`/info`, `/create`, `/claim/:token`, `/accounts`) and structured error codes. **It does not prescribe sync intervals, rate limits, or retry/backoff policies.** Therefore this design uses conservative personal-finance-app defaults and makes them user-configurable.

## Design approach

Use a lightweight **job scheduler + post-sync processors** (Approach B from brainstorming). A `SyncScheduler` lives in `AppState`, runs periodic background syncs, and executes explicit post-processors after each account sync. This gives clean separation, configurable scheduling, retry/backoff, and testable modules without introducing a full event bus.

## Scheduling, retry, and backoff

### Background sync interval
- **Default:** every 6 hours (4×/day).
- **Configurable intervals:** Off, 1h, 3h, 6h, 12h, 24h.
- **Persistence:** interval and enabled flag stored in `settings` KV under keys `simplefin.background_sync_enabled` and `simplefin.background_sync_interval_minutes`.
- **Scope:** only while the app is running. No OS-level scheduled wakeups in this slice.

### Batch sync behavior
- `sync_all_simplefin_accounts` command iterates active `simplefin_connections` and their linked `accounts`.
- Accounts sharing the same bridge access URL are fetched together via `?account=...` filters when possible.
- Sequential bridge calls use a 1-second stagger to avoid hammering the provider.
- Initial sync still fetches full history; subsequent syncs use the existing 14-day lookback.

### Retry and backoff
Per-account sync retries only on transient failures:

| Error class | Examples | Retry? | Behavior |
|---|---|---|---|
| Authentication/revocation | `403 Forbidden`, `con.auth`, `gen.auth` | No | Mark connection `status=error`, surface "Access revoked" |
| Payment required | `402 Payment Required` | No | Mark `status=error`, prompt user to renew bridge subscription |
| Transient account/connection | `act.failed`, `act.missingdata`, non-auth `con.*`, 5xx, network timeout | Yes | Exponential backoff: 1s, 2s, 4s, 8s (max 4 retries) |
| Partial errlist | `act.missingdata` for one account | Yes, per account | Retry only the affected account/connection |

Retry state is per-job and not persisted across app restarts. After max retries, update `simplefin_connections.last_error` and `status=error` without blocking other accounts.

### Error propagation
- Connection-level errors update `simplefin_connections.status` and `last_error`.
- Account-level errors are recorded in the `imports` audit row.
- UI surfaces connection health in Settings and toasts/notifications on failure.

## Data model additions

### Migration `V025__simplefin_phase4.sql`

```sql
-- Transfer links between synced transactions
CREATE TABLE transaction_transfers (
    id TEXT PRIMARY KEY,
    from_transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    to_transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    confidence TEXT NOT NULL CHECK(confidence IN ('high', 'medium', 'low')),
    detected_at TEXT NOT NULL,
    user_confirmed INTEGER NOT NULL DEFAULT 0,
    UNIQUE(from_transaction_id, to_transaction_id)
);

-- SimpleFin sync alerts (drift, errors, transfer suggestions)
CREATE TABLE simplefin_alerts (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    alert_type TEXT NOT NULL CHECK(alert_type IN ('drift', 'sync_error', 'transfer_suggestion')),
    severity TEXT NOT NULL CHECK(severity IN ('info', 'warning', 'error')),
    message TEXT NOT NULL,
    details_json TEXT,
    acknowledged_at TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_simplefin_alerts_account ON simplefin_alerts(account_id, acknowledged_at, created_at DESC);

-- Investment holdings per account per day
CREATE TABLE holdings (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    security_id TEXT NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
    quantity REAL,
    cost_basis_cents INTEGER,
    market_value_cents INTEGER,
    currency TEXT,
    as_of_date TEXT NOT NULL,
    UNIQUE(account_id, security_id, as_of_date)
);
```

The `securities` table was created in migration V024.

## Architecture

### `SyncScheduler`

New module `finsight-app/src/sync_scheduler.rs`:

```rust
pub struct SyncScheduler {
    db: Db,
    interval_minutes: Arc<AtomicU32>,
    cancellation: CancellationToken,
}

impl SyncScheduler {
    pub fn new(db: Db) -> Self;
    pub fn start(&self) -> JoinHandle<()>;
    pub fn set_interval(&self, minutes: u32);
    pub async fn sync_all_now(&self) -> Vec<SyncJobResult>;
    pub fn stop(&self);
}
```

- Stored in `AppState` and started during app setup.
- Reads settings at startup to initialize the interval.
- Manual `sync_all_simplefin_accounts` command calls `sync_all_now()`.

### Post-sync processors

Each processor is a separate function/module invoked by the scheduler after account sync:

| Processor | File | Responsibility |
|---|---|---|
| `transfer_detector` | `finsight-providers/src/simplefin/transfers.rs` | Detect likely transfer pairs |
| `holdings_importer` | `finsight-providers/src/simplefin/holdings.rs` | Upsert securities and holdings |
| `drift_monitor` | `finsight-providers/src/simplefin/drift.rs` | Compare bank vs ledger balance |

## Features

### Batch & background sync

**Commands:**
- `sync_all_simplefin_accounts() -> Vec<AccountSyncResult>`
- `get_simplefin_sync_settings() -> SimpleFinSyncSettings`
- `set_simplefin_sync_settings(settings: SimpleFinSyncSettings)`

**DTO:**
```rust
pub struct SimpleFinSyncSettings {
    pub background_sync_enabled: bool,
    pub background_sync_interval_minutes: u32,
}
```

**UI:**
- Settings → Bank connections: background-sync toggle + interval dropdown.
- Accounts screen: "Sync all" button.

### Transfer detection

**Algorithm:** After syncing account A, scan the last 30 days for transactions where:
- Same absolute amount.
- Opposite sign.
- Posted within ±3 days of a transaction in another linked account.
- Description contains transfer-like keywords ("transfer", "zelle", "venmo", "wire", etc.) or there is a unique amount match.

**Confidence:**
- `high`: same amount, opposite sign, ≤1 day, keyword match.
- `medium`: same amount, opposite sign, ≤3 days.
- `low`: same amount, opposite sign, ≤7 days, fuzzy description.

**Commands:**
- `list_transfer_suggestions() -> Vec<TransferSuggestion>`
- `confirm_transfer_link(id: String)` / `reject_transfer_link(id: String)`

**UI:** Inbox/Alerts shows unconfirmed transfer suggestions.

### Holdings and securities

**Source:** SimpleFin account `extra` may contain a `holdings` array:
```json
{
  "holdings": [
    { "ticker": "VTI", "name": "Vanguard Total Stock", "quantity": 10.5, "unit_price": "250.00", "currency": "USD" }
  ]
}
```

If absent and account type is Investment, store a zero-quantity placeholder holding.

**Processor behavior:**
- Upsert securities keyed by `(connection_id, ticker_symbol)`.
- Upsert a holdings row with `as_of_date = today` and `market_value_cents = quantity * unit_price`.

**Commands:**
- `list_securities() -> Vec<Security>`
- `list_holdings(account_id: String) -> Vec<Holding>`

**UI:** Accounts screen shows holdings count / total market value for investment accounts.

### Balance-drift alerts

**Computation:**
```
ledger_balance = SUM(transactions.amount_cents) for account
reported_balance = latest account_balances.balance_cents from SimpleFin

drift_cents = ledger_balance - reported_balance
```

**Severity thresholds:**
- `error`: |drift| > 500 cents ($5.00).
- `warning`: 1 ≤ |drift| ≤ 500 cents.
- `info`: drift = 0.

Create a `simplefin_alerts` row when drift crosses a threshold after sync. Do not duplicate an unacknowledged alert for the same account within the same day.

**Commands:**
- `list_simplefin_alerts() -> Vec<SimpleFinAlert>`
- `acknowledge_simplefin_alert(id: String)`

**UI:** Inbox/Alerts shows unacknowledged drift alerts; account rows show a drift badge.

## Testing strategy

- Unit tests for retry/backoff classification in `SyncScheduler`.
- In-memory DB tests for transfer detection with seeded transactions.
- In-memory DB tests for drift computation.
- Wiremock tests for batch sync using HTTPS mock server (or mark ignored if TLS termination unavailable).
- Frontend tests for Settings sync controls and alert list.

## Success criteria

1. User can enable/disable background sync and choose an interval.
2. `Sync all` syncs every linked SimpleFin account and runs post-processors.
3. Transfers between linked accounts generate suggestions with confidence scores.
4. Investment accounts persist securities and daily holdings.
5. Bank-reported vs ledger drift creates alerts with correct severity.
6. All existing tests still pass; new tests cover the four feature areas.
