# FinSight

Your AI-powered financial copilot — a quiet way to understand, plan, and master your money.

Dark-first and encrypted at rest, with AI-assisted planning and categorization. Built on the timeless principles of *The Richest Man in Babylon*, *The Total Money Makeover*, *I Will Teach You to Be Rich*, *The Psychology of Money*, *Rich Dad Poor Dad*, and *Think and Grow Rich*.

### Privacy

Your data lives only in an encrypted SQLCipher database on your machine (key in the OS keychain). Nothing is uploaded to any FinSight server — there isn't one.

The one time data leaves your device is when you opt into a **cloud** AI provider (OpenAI-compatible or Anthropic) for categorization or the Copilot:

- **Auto-categorization** sends the *merchant description and amount* of each **uncategorized** transaction to your chosen provider to pick a category. Reference numbers and the names of people in e-transfers are **redacted** first; balances, account numbers, and totals are never sent. Toggle it off in Settings → Agent, or pick a local **Ollama** provider to keep everything on-device.
- **The Copilot** answers your questions using tools that read your local data; when you ask it something, the relevant figures/merchants it needs are sent to your provider as part of the conversation.

Settings → Agent shows exactly what is sent, and Settings → Data & backups lets you snapshot and restore your encrypted database at any time.

## Philosophy

FinSight is designed around one goal: helping you become the master of your own finances. The agentic Copilot applies proven principles automatically:

- **Pay Yourself First** — save ≥10% before anything else (Babylon / Ramsey)
- **Conscious Spending** — tag every category as a Need, Want, Saving, or Investment and see your allocation at a glance (Sethi)
- **Debt Snowball** — smallest-balance-first payoff order keeps momentum (Ramsey)
- **Emergency Fund First** — 3–6 months of expenses as the foundation of any plan (Ramsey / Sethi)
- **Compound Growth** — project 10/20/30-year wealth from current savings rate (Hill / Kiyosaki)
- **Behaviour over math** — the Copilot surfaces patterns and nudges, not just numbers (Housel)

## What's built

| Screen | Status | Notes |
|--------|--------|-------|
| Today | ✅ | Net/income/expenses stats, savings rate card (colour-coded), category stream bar, privacy mode |
| Copilot | ✅ | AI financial planner — goal-aware plans, action bundles, conversational Q&A, nudges, and native **generative-UI blocks** (typed, validated finance cards — spending review, accounts overview, drivers, action plans — rendered in-app, not raw markdown) |
| Insights | ✅ | AI anomaly cards, spending patterns, agent memory, needs-review feed |
| Accounts | ✅ | Manual accounts + assets/liabilities, balance history, net worth, CSV import |
| Transactions | ✅ | Search, filter tabs (needs review / anomalies / no category), drawer edit |
| Budget | ✅ | Envelope grid, To Budget tracker, Conscious Spending allocation donut |
| Categories | ✅ | Month / vs-last / YTD scope, spending-type picker (Need/Want/Saving/Investment) |
| Recurring | ✅ | Calendar view with day-detail panel, list view, subscriptions |
| Goals | ✅ | 4 goal types, pace chip, what-if slider, emergency fund quick-fill, compound growth projector |
| Reports | ✅ | 12-month bar + net line charts, category/merchant tables |
| Scenarios | ✅ | Natural-language what-if forecasting with LLM-powered projections |
| Recipes | ✅ | Trusted automation recipes (monthly budget draft, weekly cleanup, goal check, etc.) |
| Journey | ✅ | 7-milestone financial journey from stability to freedom, with Copilot entry points |
| Rules | ✅ | Pattern rules, agent auto-categorization, toggle enable/disable |
| Settings | ✅ | LLM provider config (Ollama / OpenAI-compat / Anthropic), test connection |
| Onboarding | ✅ | Connect accounts (CSV / manual / SimpleFin), category starter pack, provider setup |

## Development

```bash
# Install JS deps (from repo root)
pnpm install

# Start Tauri dev (backend + frontend hot reload)
pnpm tauri:dev

# Frontend only (no Tauri, for UI iteration)
cd ui && npm run dev

# Run all tests
cargo test --workspace
cd ui && npx vitest run

# After adding a Tauri command — regenerate TS bindings (run from repo root)
cargo run -p finsight-tauri --bin export_bindings
```

## Architecture

```
FinSight/
├── crates/
│   ├── finsight-core/      # DB schema, migrations (SQLCipher), repos, models
│   ├── finsight-providers/ # CSV import parsers, LLM provider HTTP clients
│   ├── finsight-agent/     # Copilot planner, context engine, categorizer, anomaly detection, recipe runner
│   ├── finsight-app/       # Tauri commands (API surface), app state
│   └── finsight-tauri/     # Tauri entry point + specta bindings export binary
└── ui/
    ├── src/
    │   ├── api/            # Generated bindings + tanstack-query hooks
    │   ├── components/     # Sidebar, CommandPalette, Drawer, TransactionDrawer, CopilotNudge, …
    │   ├── screens/        # One file per screen
    │   └── styles/         # tokens.css (design tokens) + app.css (component classes)
    └── …
```

**Stack:** Rust/Tauri 2 · React 18 + TypeScript + Vite · SQLite/SQLCipher · tanstack-query · sonner toasts · zod + react-hook-form

**Storage:** `~/<app-data>/data.sqlcipher` encrypted with a key stored in the OS keychain. Debug/dev builds use an isolated `<identifier>.dev` data directory so `pnpm tauri:dev` never touches the real production database; the WAL is checkpointed on clean exit.

## Adding a Tauri command

1. Write `pub async fn my_cmd(state: tauri::State<'_, AppState>) -> AppResult<T>` in `crates/finsight-app/src/commands/`
2. Add `#[tauri::command]` and `#[specta::specta]` attributes
3. Register in `build_specta_builder()` in `crates/finsight-app/src/lib.rs`
4. Run `cargo run -p finsight-tauri --bin export_bindings` from repo root
5. The new command appears in `ui/src/api/bindings.ts` and `ui/src/api/client.ts`

## CSS conventions

- Design tokens: `var(--ink)`, `var(--ink-mute)`, `var(--ink-faint)`, `var(--line)`, `var(--elevated)`, `var(--accent)`, `var(--negative)`, `var(--surface-2)` — see `ui/src/styles/tokens.css`
- Component classes: `.card`, `.chip`, `.btn`, `.tbl`, `.stat`, `.eyebrow`, `.toolbar`, `.stream`, `.goal-bar`, `.stub`, `.muted`, `.num`, `.money` — see `ui/src/styles/app.css`
- Never use hardcoded hex colors in components

## What's next

All screens are built. The two audit documents in `docs/audits/` are the
living record of what was found and fixed:
[`2026-07-10-finsight-product-audit.md`](docs/audits/2026-07-10-finsight-product-audit.md)
(15 ranked P0–P2 findings + 6 P3 items, each with its resolving commit) and
[`2026-07-10-completeness-and-cross-user-ownership.md`](docs/audits/2026-07-10-completeness-and-cross-user-ownership.md)
(cross-user ownership shares + a fresh real-data sweep). As of 2026-07-13 every
item in both documents is resolved and verified — against `samples/` via the
rerunnable probe (`crates/finsight-app/tests/audit_probe.rs`) and, for the UI
layer, against the actual compiled Tauri app. New gaps will be added to those
documents as they're found, not tracked here.

Since then (2026-07-15): the Copilot gained native generative-UI blocks (with a
server-synthesis + structured-output + heal robustness net); dev/prod databases
were isolated to stop a recurring corruption; and transfer detection got a
further pass (F0) — nameless `INTERNET TRANSFER` legs now pair across accounts,
e-transfers to a configured household member read as internal, and the pairing
is now reproducible regardless of row-id order. The honest remaining transfer
gap — genuinely ambiguous bidirectional person-to-person e-transfers — belongs
to the confirm-once review workflow, not auto-detection.

Longer-horizon ideas beyond the audits:

1. **More Copilot recipes** — custom user-defined automation templates
2. **Debt avalanche mode** — highest-interest-first as an alternative to snowball
3. **Journey milestone notifications** — celebrate when a milestone is reached
4. **Export / reports** — PDF summaries of monthly spending and progress
