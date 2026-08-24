# FinSight

Your private, self-hosted financial copilot — a quiet way to understand, plan,
and master your money.

FinSight combines encrypted personal-finance storage, account and transaction
management, budgeting, goals, reporting, and an AI-assisted Copilot. The
primary deployment is a server you operate yourself, accessed from a browser
or an installable PWA (desktop and mobile via Tailscale — no native binary).

Built on principles from *The Richest Man in Babylon*, *The Total Money
Makeover*, *I Will Teach You to Be Rich*, *The Psychology of Money*, *Rich Dad
Poor Dad*, and *Think and Grow Rich*.

## How FinSight runs

| Component | What it does | Where data lives |
|---|---|---|
| `finsight-server` | Serves the UI, authenticated RPC API, CSV uploads, SSE events, and `GET /api/openapi.json` | One encrypted SQLCipher database per user under the server's data directory |
| Browser / PWA | Connects to the server over same-origin HTTP/SSE | A seven-day, read-only IndexedDB query cache for offline viewing; purged on logout or authentication failure |

The PWA is the desktop app: install from the browser (or `Add to Dock` on
macOS) and it runs standalone with its own icon and window, offline read cache
included. No separate binary — `docker compose up` plus Tailscale/Caddy is the
whole install.

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
docker compose up -d
docker compose logs -f finsight
```

This pulls the public multi-architecture image from GitHub Container Registry;
it does not compile FinSight on your server. To build this checkout instead,
use `docker compose -f docker-compose.yml -f docker-compose.build.yml up
--build -d`.

Copy `finsight.env.example` to `.env` when you need to pin an image version,
change the host port, test over bare LAN HTTP, or declare a reverse-proxy
origin. Versioned GitHub Releases include a Compose file and environment
example already pinned to the matching server image.

Open `http://localhost:8674` for a local smoke test. To complete setup and
sign in on the Docker host, no cookie override is needed: browsers treat
`localhost` and `127.0.0.1` as trustworthy local origins. The first account
created becomes the administrator and receives a one-time recovery key. Save it
before continuing.

For plain HTTP access from another device over a LAN address, set
`FINSIGHT_COOKIE_SECURE=0`; that is a limited, non-PWA test mode because browsers
do not treat a bare LAN HTTP origin as secure. Restore the default value of `1`
before putting FinSight behind HTTPS.

For normal use, put FinSight behind HTTPS and keep secure cookies enabled. The
full Tailscale, Caddy, LAN TLS, PWA installation, backup, and upgrade
instructions are in [docs/self-hosting.md](docs/self-hosting.md). An optional
`deploy/compose.split.yaml.example` shows how to front the same image with
nginx (`web` + `api`) if you later want a split deployment — no client change.

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
`FINSIGHT_DATA_DIR` is set.

Validation commands:

```bash
pnpm typecheck
pnpm --filter ui test
cargo test --workspace
pnpm build

# Regenerate the OpenAPI contract + TypeScript client after changing the API
pnpm openapi   # cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen
```

## Architecture

```text
Browser / PWA
      │
 HTTP RPC + CSV upload + SSE (+ GET /api/openapi.json)
      │
crates/finsight-server  (Axum, serves ui/dist)
      │
crates/finsight-api  (transport-agnostic handlers + ApiState)
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
│   ├── finsight-openapi/    # Typed OpenAPI spec (utoipa) — COMMANDS + build_openapi()
│   ├── finsight-server/     # Axum auth, RPC, uploads, SSE, static UI, user runtimes
│   └── finsight-eval/       # Evaluation fixtures and runners
├── deploy/
│   ├── compose.split.yaml.example  # optional web (nginx) + api split
│   └── docker/nginx.conf
└── ui/
    └── src/
        ├── api/             # Generated openapi.ts + openapiClient, auth, query hooks
        ├── components/      # Shared UI, auth/offline gates, Copilot renderers
        ├── pwa/             # IndexedDB persistence and online state
        ├── screens/         # Product, server-auth, admin screens
        └── styles/          # Design tokens and component styles
```

The generated `ui/src/api/openapi.ts` (from `openapi.json` via
`openapi-typescript`) is the frontend contract. `ui/src/api/openapiClient.ts`
(`openapi-fetch`) is the typed transport over `POST /api/rpc/{cmd}` and
`GET /api/events` — every hook imports `api` from there, with `Result<T,AppError>`
envelope and 401 → `FINSIGHT_AUTH_REQUIRED` handling (the sole transport; no
Tauri shim remains).

### Adding or changing a shared command

1. Implement the body as `pub async fn my_cmd(state: &ApiState, ...)` in
   `crates/finsight-api/src/commands/`.
2. Add `#[utoipa::path]` + `ToSchema` on the handler/DTOs (see
   `crates/finsight-openapi/src/lib.rs` `COMMANDS` — keep it sorted and
   identical to `dispatch.rs` `SUPPORTED`).
3. Add the dispatcher arm in `crates/finsight-server/src/dispatch.rs`
   (`rpc_routes!(api, events, cmd, p, c: …)` — use `arg(&p, "camelCase")`).
4. Run `pnpm openapi` (`cargo run -p finsight-openapi --bin export_openapi`
   + `pnpm --filter ui openapi:gen`) and `cargo test -p finsight-server --test parity`
   + `cargo test -p finsight-openapi` (spec must stay in sync with `SUPPORTED`).
5. If the DTO shape changed, `pnpm typecheck` will fail until hooks are updated.

## Data layout

The default Docker data directory is `/data`; native development defaults to
`./data`:

```text
data/
├── users.db                         # account registry, wrapped keys, session hashes
├── session.key                      # wraps persisted sessions; back this up
└── users/<user-uuid>/
    ├── data.sqlcipher               # this user's financial data and secrets
    ├── backups/                     # manual and pre-migration snapshots
    └── imports/                     # authenticated CSV upload staging
```

Per-user runtimes are created lazily, single-flighted for concurrent requests,
and evicted after 30 minutes of inactivity when no SSE client is attached.
Sessions have a sliding 30-day lifetime and survive server restarts. The cookie
token is stored only on the client; `users.db` stores its hash and a database key
wrapped by `/data/session.key`. Unwrapped database keys still exist only in
server memory. Logout, password recovery, user deletion, and “sign out other
devices” revoke the corresponding persisted rows.

## CSS conventions

- Use the tokens in `ui/src/styles/tokens.css`; do not hardcode component colors.
- Reuse shared components and the utility classes in `ui/src/styles/app.css`.
- Amounts that must respect privacy mode use the `money` class.

## Project status

The self-hosted server, multi-user encryption and recovery flow, browser/PWA
transport, Docker deployment (single-container default, optional nginx split),
offline read cache, and OpenAPI contract (`GET /api/openapi.json`,
`openapi-typescript` client) are implemented.
[docs/self-hosting.md](docs/self-hosting.md) documents current deployment and
operational limits.

The dated files in `docs/audits/`, `docs/handoffs/`, and
`docs/superpowers/` are historical design and verification records. The active
Copilot and agentic-finance roadmap is
[docs/agentic-finance-todo.md](docs/agentic-finance-todo.md).
