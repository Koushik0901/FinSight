# FinSight

Your AI-powered financial copilot — a quiet way to understand, plan, and master your money.

Dark-first, encrypted at rest, AI-assisted planning and categorization with no data leaving your machine unless you opt in. Built on the timeless principles of *The Richest Man in Babylon*, *The Total Money Makeover*, *I Will Teach You to Be Rich*, *The Psychology of Money*, *Rich Dad Poor Dad*, and *Think and Grow Rich*.

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
| Copilot | ✅ | AI financial planner — goal-aware plans, action bundles, conversational Q&A, nudges |
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

**Storage:** `~/<app-data>/data.sqlcipher` encrypted with a key stored in the OS keychain.

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

The core app is feature-complete. Potential next areas:

1. **More Copilot recipes** — custom user-defined automation templates
2. **Debt avalanche mode** — highest-interest-first as an alternative to snowball
3. **Journey milestone notifications** — celebrate when a milestone is reached
4. **Export / reports** — PDF summaries of monthly spending and progress
