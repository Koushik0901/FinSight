# FinSight

Local-first personal finance — a quiet way to understand your money.

Dark-first, encrypted at rest, AI-assisted categorization with no data leaving your machine unless you opt in.

## What's built

| Screen | Status | Notes |
|--------|--------|-------|
| Today | ✅ | Net/income/expenses stats, category stream bar, privacy mode |
| Insights | ✅ | AI anomaly cards, spending patterns, needs-review feed |
| Accounts | ✅ | Manual accounts, balance history, CSV import |
| Transactions | ✅ | Search, filter tabs (needs review / anomalies / no category), drawer edit |
| Budget | ✅ | Envelope grid, To Budget tracker, by-group/stress/size/activity sort |
| Categories | ✅ | Month / vs-last / Year-to-date scope, budget column |
| Recurring | ✅ | Calendar view with day-detail panel, list view, subscriptions |
| Goals | ✅ | 4 goal types, pace chip (Ahead/On track/Needs attention), what-if slider |
| Reports | ✅ | 12-month bar + net line charts, category/merchant tables |
| Rules | ✅ | Pattern rules, agent auto-categorization, toggle enable/disable |
| Settings | ✅ | LLM provider config (Ollama / OpenAI-compat / Anthropic), test connection |
| Onboarding | ✅ | Sample data seeding, category starter pack, provider setup |

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
│   ├── finsight-agent/     # LLM provider trait, categorizer, anomaly detection
│   ├── finsight-app/       # Tauri commands (API surface), app state
│   └── finsight-tauri/     # Tauri entry point + specta bindings export binary
└── ui/
    ├── src/
    │   ├── api/            # Generated bindings + tanstack-query hooks
    │   ├── components/     # Sidebar, CommandPalette, Drawer, TransactionDrawer, …
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

See `docs/TODO.md` for the full gap analysis against the Plutus design reference. Top remaining items:

1. **Scenarios screen** — natural-language what-if forecasting (§1)
2. **Rules: agent proposals + manual builder** (§11a, §11b)
3. **Command palette: Ask the agent** (§14a)
4. **Today: net-worth chart + upcoming recurring** (§3a, §3c)
5. **Accounts: manual assets + liabilities** (§4a, §4b)
