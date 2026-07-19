# FinSight

Your private, self-hosted financial copilot — a quiet way to understand, plan,
and master your money.

FinSight combines encrypted personal-finance storage, account and transaction
management, budgeting, goals, reporting, and an AI-assisted Copilot. The
primary deployment is a server you operate yourself, accessed from a browser,
an installable PWA, or the thin native desktop shell.

Built on principles from *The Richest Man in Babylon*, *The Total Money
Makeover*, *I Will Teach You to Be Rich*, *The Psychology of Money*, *Rich Dad
Poor Dad*, and *Think and Grow Rich*.

## How FinSight runs

| Component | What it does | Where data lives |
|---|---|---|
| `finsight-server` | Serves the UI, authenticated RPC API, CSV uploads, and SSE events | One encrypted SQLCipher database per user under the server's data directory |
| Browser / PWA | Connects to the server over same-origin HTTP/SSE | A seven-day, read-only IndexedDB query cache for offline viewing; purged on logout or authentication failure |
| Desktop shell | Remembers a server URL, then loads that server in a Tauri webview | The URL is stored in the OS keychain; financial data remains on the server, with the same web cache as the PWA |

The desktop binary is intentionally a thin client. It no longer opens a local
financial database or exposes the full Tauri command surface. On first launch
it asks for a FinSight server URL, verifies `/api/health`, saves the URL, and
navigates to the server-hosted app.

## Privacy and security

FinSight does not operate a hosted service. When you self-host it, financial
data is stored on infrastructure you control:

- Each FinSight user gets a separate SQLCipher database and encryption key.
- `users.db` is a plain SQLite account registry containing usernames, Argon2id
  password verifiers, and wrapped database keys. It contains no financial
  records, plaintext passwords, or plaintext database keys.
- The database key is wrapped independently by the user's password and by a
  printable recovery key. Recovery resets the password, rotates the recovery
  key, and revokes the user's existing sessions.
- LLM API keys and SimpleFIN access URLs are stored inside that user's encrypted
  database, not in a process-global server keychain slot.
- Session cookies are `HttpOnly` and `SameSite=Lax`. Production HTTPS deployments
  should keep `FINSIGHT_COOKIE_SECURE=1`.
- The server's `/data` volume is the source of truth. Browser/PWA caches are for
  read-only offline access and are cleared when a session ends.

Data leaves your server only when you opt into an external integration:

- **Cloud AI providers:** auto-categorization sends the redacted merchant
  description and amount of uncategorized transactions to the configured
  provider. The Copilot sends the relevant financial context needed to answer
  a question. Use Ollama to keep inference on infrastructure you control.
- **SimpleFIN:** FinSight exchanges the stored access URL with SimpleFIN when
  you explicitly connect and synchronize accounts.

Settings → Agent explains the AI-provider data flow. Settings → Data & backups
creates and restores encrypted per-user snapshots on the server. For disaster
recovery, also back up the complete `/data` volume so `users.db` and every
user's encrypted database stay together.

## Philosophy

FinSight is designed around one goal: helping you become the master of your own
finances. The Copilot applies proven principles automatically:

- **Pay Yourself First** — save ≥10% before anything else (Babylon / Ramsey)
- **Conscious Spending** — tag every category as a Need, Want, Saving, or Investment and see your allocation at a glance (Sethi)
- **Debt Snowball** — smallest-balance-first payoff order keeps momentum (Ramsey)
- **Emergency Fund First** — 3–6 months of expenses as the foundation of any plan (Ramsey / Sethi)
- **Compound Growth** — project 10/20/30-year wealth from the current savings rate (Hill / Kiyosaki)
- **Behaviour over math** — surface patterns and nudges, not just numbers (Housel)

## What's built

| Screen | Status | Notes |
|---|---|---|
| Today | ✅ | Net/income/expenses stats, savings rate, category stream, privacy mode |
| Copilot | ✅ | Goal-aware planning, conversational Q&A, action bundles, streaming, and typed generative-UI finance cards |
| Insights | ✅ | Anomalies, spending patterns, agent memory, and needs-review feed |
| Accounts | ✅ | Manual and SimpleFIN accounts, assets/liabilities, balance history, net worth, and CSV import |
| Transactions | ✅ | Search, filters, review queues, transfers, splits, and drawer editing |
| Budget | ✅ | Envelope planning, To Budget tracker, carryover, and Conscious Spending allocation |
| Categories | ✅ | Month / prior-period / YTD scopes, groups, guidance, and spending types |
| Recurring | ✅ | Calendar and list views, subscriptions, and recurring-payment detection |
| Goals | ✅ | Goal tracking, contribution history, emergency-fund quick fill, and compound-growth projection |
| Reports | ✅ | Monthly trends, category and merchant tables, review snapshots, and exports |
| Scenarios | ✅ | Natural-language what-if forecasting with deterministic finance tools |
| Recipes | ✅ | Trusted automation recipes for budgets, cleanup, goals, and reviews |
| Journey | ✅ | Seven financial milestones from stability to freedom, with Copilot entry points |
| Rules | ✅ | Pattern rules, treatment rules, agent categorization, and enable/disable controls |
| Settings | ✅ | Provider configuration, encrypted backups/restores, exports, server account controls, and admin user management |
| Onboarding | ✅ | Account-first setup, manual/SimpleFIN accounts, CSV history, categories, and provider setup |

## Self-hosting quick start

Prerequisites: Docker Engine and Docker Compose.

```bash
git clone https://github.com/Koushik0901/FinSight.git
cd FinSight
docker compose up --build -d
docker compose logs -f finsight
```

Open `http://localhost:8674` for a local smoke test. To complete setup and
sign in over bare HTTP, temporarily set `FINSIGHT_COOKIE_SECURE=0` in
`docker-compose.yml`; restore it to `1` when HTTPS is configured. The first
account created becomes the administrator and receives a one-time recovery key.
Save it before continuing.

For normal use, put FinSight behind HTTPS and keep secure cookies enabled. The
full Tailscale, Caddy, LAN TLS, PWA installation, backup, upgrade, and desktop
shell instructions are in [docs/self-hosting.md](docs/self-hosting.md).

## Development

Install dependencies from the repository root:

```bash
pnpm install
```

Run server mode with hot-reloading frontend assets in two terminals:

```bash
# Terminal 1: API/SSE server on http://localhost:8674
cargo run -p finsight-server

# Terminal 2: Vite on http://localhost:5173; /api proxies to :8674
pnpm dev
```

The development server stores data in `./data` unless
`FINSIGHT_DATA_DIR` is set. To exercise the native thin shell, stop the
standalone Vite process, leave `finsight-server` running, and run:

```bash
pnpm tauri:dev
```

Enter `http://localhost:8674` on the Connect screen.

Validation commands:

```bash
pnpm typecheck
pnpm --filter ui test
cargo test --workspace
pnpm build

# Regenerate TypeScript bindings after changing the command contract
pnpm bindings
```

## Architecture

```text
Browser / PWA / navigated desktop shell
                  │
       HTTP RPC + CSV upload + SSE
                  │
        crates/finsight-server
                  │
          crates/finsight-api
                  │
     ┌────────────┼──────────────┐
finsight-core  finsight-agent  finsight-providers
```

```text
FinSight/
├── crates/
│   ├── finsight-core/       # SQLCipher DB, migrations, models, repositories
│   ├── finsight-providers/  # CSV parsers and LLM HTTP providers
│   ├── finsight-agent/      # Copilot, finance tools, categorizer, anomalies, recipes
│   ├── finsight-api/        # Transport-agnostic command bodies and ApiState
│   ├── finsight-app/        # Codegen-only Tauri wrappers for the shared contract
│   ├── finsight-server/     # Axum auth, RPC, uploads, SSE, static UI, user runtimes
│   └── finsight-eval/       # Evaluation fixtures and runners
├── src-tauri/               # Thin desktop shell + bindings exporter
└── ui/
    └── src/
        ├── api/             # Generated bindings, HTTP shim, auth, query hooks
        ├── components/      # Shared UI, auth/offline gates, Copilot renderers
        ├── pwa/             # IndexedDB persistence and online state
        ├── screens/         # Product, server-auth, admin, and desktop-connect screens
        └── styles/          # Design tokens and component styles
```

The generated `ui/src/api/bindings.ts` remains the frontend command contract.
In server mode, `ui/src/api/httpBackend.ts` implements the Tauri invoke/event
shape over `POST /api/rpc/{cmd}` and `GET /api/events`, so screens and hooks use
the same client API in every runtime.

### Adding or changing a shared command

1. Implement the command body in `crates/finsight-api/src/commands/`.
2. Add or update its thin `#[tauri::command]` / `#[specta::specta]` wrapper in
   `crates/finsight-app/src/commands/`.
3. Register the wrapper in `build_specta_builder()` in
   `crates/finsight-app/src/lib.rs`.
4. Add the server dispatcher arm and command name in
   `crates/finsight-server/src/dispatch.rs`. Dispatcher argument keys must use
   the camelCase keys emitted by the generated bindings.
5. Run `pnpm bindings` and `cargo test -p finsight-server --test parity`.

## Data layout

The default Docker data directory is `/data`; native development defaults to
`./data`:

```text
data/
├── users.db                         # account registry + wrapped keys
└── users/<user-uuid>/
    ├── data.sqlcipher               # this user's financial data and secrets
    ├── backups/                     # manual and pre-migration snapshots
    └── imports/                     # authenticated CSV upload staging
```

Per-user runtimes are created lazily, single-flighted for concurrent requests,
and evicted after 30 minutes of inactivity when no SSE client is attached.
Sessions use a sliding 30-day in-memory TTL, so a server restart requires users
to sign in again.

## CSS conventions

- Use the tokens in `ui/src/styles/tokens.css`; do not hardcode component colors.
- Reuse shared components and the utility classes in `ui/src/styles/app.css`.
- Amounts that must respect privacy mode use the `money` class.

## Project status

The self-hosted server, multi-user encryption and recovery flow, browser/PWA
transport, Docker deployment, offline read cache, and thin desktop shell are
implemented. [docs/self-hosting.md](docs/self-hosting.md) documents current
deployment and operational limits.

The dated files in `docs/audits/`, `docs/handoffs/`, and
`docs/superpowers/` are historical design and verification records. The active
Copilot and agentic-finance roadmap is
[docs/agentic-finance-todo.md](docs/agentic-finance-todo.md).
