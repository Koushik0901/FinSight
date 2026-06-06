# Wave B — All Remaining TODO Items
**Date:** 2026-06-05  
**Status:** Approved  
**Scope:** 13 TODO items across 7 screens + CommandPalette, shipped in one wave

---

## Overview

Wave B ships every remaining medium/high-value item from `docs/TODO.md`. The work breaks into:
- 1 backend task (4 new Rust commands + 1 core repo function, no migration needed)
- 1 hook task (3 new hook files + additions to existing files)
- 11 frontend tasks across 7 screens

**Green bar target (before wave start):** 72 frontend tests, all Rust tests passing, 0 TypeScript errors.

---

## Architecture: Backend additions

All backend work lives in existing crates — no new SQLite migration is needed.

### 1a. `update_goal_monthly` (`crates/finsight-app/src/commands/budget.rs`)

**Core repo change:** Add to `crates/finsight-core/src/repos/goals.rs`:
```rust
pub fn set_monthly_cents(conn: &mut Connection, id: &str, monthly_cents: i64) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET monthly_cents = ?1 WHERE id = ?2",
        params![monthly_cents, id],
    )?;
    Ok(())
}
```

**Tauri command** (in `commands/budget.rs`):
```rust
#[tauri::command]
#[specta::specta]
pub async fn update_goal_monthly(
    state: tauri::State<'_, AppState>,
    id: String,
    monthly_cents: i64,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| goals::set_monthly_cents(conn, &id, monthly_cents))
        .await
        .map_err(AppError::from)
}
```

Register in `build_specta_builder()` in `crates/finsight-app/src/lib.rs`.

### 1b. Data export commands (`crates/finsight-app/src/commands/settings.rs` — new file)

Two commands that use `tauri_plugin_dialog` to show a native save dialog, then write the file:

```rust
#[tauri::command]
#[specta::specta]
pub async fn export_all_data_json(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> AppResult<()>
```
- Opens a save dialog (defaulting to `finsight-export.json`)
- If user cancels, returns `Ok(())`
- Queries all accounts, transactions (last 2 years), categories, goals, rules
- Serializes as a JSON object `{ exportedAt, accounts, transactions, categories, goals, rules }`
- Writes to chosen path

```rust
#[tauri::command]
#[specta::specta]
pub async fn export_all_data_csv(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> AppResult<()>
```
- Opens a save dialog (defaulting to `finsight-transactions.csv`)
- Writes transactions as CSV: `date,merchant,category,amount_dollars,notes`
- Amount formatted as decimal (e.g. `-42.99` for expense, `1200.00` for income)

Both commands are registered in `build_specta_builder()`.

### 1c. Currency KV commands (`commands/settings.rs` — same new file)

```rust
#[tauri::command]
#[specta::specta]
pub async fn get_currency(state: tauri::State<'_, AppState>) -> AppResult<String>
// Returns settings KV "display_currency", default "USD"

#[tauri::command]
#[specta::specta]
pub async fn set_currency(state: tauri::State<'_, AppState>, currency: String) -> AppResult<()>
// Stores in settings KV "display_currency"
```

After all commands are written and registered, run:
```
cargo run -p finsight-tauri --bin export_bindings
```
This unblocks all frontend tasks.

---

## Architecture: Hook additions

### Hook file: `ui/src/api/hooks/recurring.ts` (new)

```typescript
export function useRecurring() {
  return useQuery<RecurringItem[]>({
    queryKey: ["recurring"],
    queryFn: async () => { /* commands.listRecurring() */ },
    staleTime: 5 * 60_000,
  });
}
```

### Hook additions to `ui/src/api/hooks/budget.ts`

```typescript
export function useUpdateGoalMonthly() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, monthlyCents }: { id: string; monthlyCents: number }) =>
      commands.updateGoalMonthly(id, monthlyCents).then(unwrap),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["goals"] }),
  });
}
```

### Hook file: `ui/src/api/hooks/settings.ts` (new)

```typescript
export function useDefaultCurrency() {
  return useQuery<string>({
    queryKey: ["currency"],
    queryFn: async () => { /* commands.getCurrency() */ },
    staleTime: Infinity,
  });
}

export function useSetCurrency() {
  const qc = useQueryClient();
  const setCurrencyTweak = useTweaks((s) => s.setCurrency);
  return useMutation({
    mutationFn: (currency: string) => commands.setCurrency(currency).then(unwrap),
    onSuccess: (_, currency) => {
      setCurrencyTweak(currency);
      qc.invalidateQueries({ queryKey: ["currency"] });
    },
  });
}

export function useExportJson() {
  return useMutation({ mutationFn: () => commands.exportAllDataJson().then(unwrap) });
}

export function useExportCsv() {
  return useMutation({ mutationFn: () => commands.exportAllDataCsv().then(unwrap) });
}
```

### `ui/src/state/tweaks.ts` additions

Add `currency: string` (default `"USD"`) and `setCurrency: (c: string) => void` to the Zustand store state and persist shape.

---

## Feature designs

### §3b — Smart Sweep card (`Today.tsx`)

**Position:** Between the net-worth chart section and the stat row.

**Visibility:** `totals.netCents > 5000 && !dismissed`. The `dismissed` state is local (`useState(false)`), not persisted — resets each session.

**Data dependency:** `useGoals()` — pulls first active goal name for the CTA button.

**Layout:**
```
┌─ ✦ Opportunity ──────────────────────────────────────────┐
│  You have $1,240 unallocated this month.                  │
│  [Park in Italy Fund]  [Assign to a goal…]  [Dismiss]     │
└──────────────────────────────────────────────────────────┘
```
- Card has `border: 1px solid var(--accent)` left-accent style
- "Park in [Goal]" calls `useUpdateGoalBalance({ id: firstGoal.id, currentCents: firstGoal.currentCents + totals.netCents })`, then shows `toast.success("Parked in [Goal]")`
- "Assign to a goal…" navigates to `/goals` + `onClose()` (no persistence)
- "Dismiss" calls `setDismissed(true)`
- If no goals exist, only show "Assign to a goal…" and "Dismiss"
- Guard: wrap in `try/catch` with `toast.error` on the Park button

### §3c — Upcoming recurring chips (`Today.tsx`)

**Position:** Between the category stream legend and the AgentActivityFeed.

**Data:** `useRecurring()` filtered to items where `new Date(item.nextExpected) - new Date() <= 7 * 86400000` and `>= 0` (not past-due).

**Layout:** Horizontal flex row (wrapping if many items):
```
[●] Spotify $9  in 2 days   [●] Netflix $15  tomorrow   [●] Rent $2,000  in 5 days   See all →
```
Each chip: `categoryColor` dot + `merchantRaw` (truncated to 18 chars) + `money(Math.abs(lastAmountCents))` + days-until label.

**Days-until label:**
- 0 days: "today"
- 1 day: "tomorrow"
- N days: "in N days"

**"See all →"** navigates to `/recurring`.

**Empty state:** render nothing (no placeholder).

### §3d — Runway stat (`Today.tsx`)

**Position:** Replaces the 4th "Accounts" stat card in the stat-row div.

**Computation:**
```typescript
const dayOfMonth = now.getDate();
const avgDailyBurn = totals.expenseCents / dayOfMonth;  // cents/day
// netWorth already in scope from useNetWorth() at top of Today.tsx
const runwayDays = avgDailyBurn > 0 ? Math.round(netWorth / avgDailyBurn) : null;
```

**Display:**
```
label: "Runway"
value: runwayDays !== null ? `${runwayDays.toLocaleString()}` : "—"
sub:   runwayDays !== null ? "days · at current burn" : "no burn data"
```

Color of `value`: `var(--negative)` if `runwayDays < 30`, default otherwise.

**Edge cases:** If `netWorth <= 0`, runway can be 0 or negative — clamp to `Math.max(0, runwayDays)`.

### §6c — AI insight sentence (`Categories.tsx`)

**Position:** Below the scope toolbar, above the category table. Only shown when `scope === "month"` and `cats.some(c => c.lastMonthCents > 0)`.

**Computation:**
```typescript
const withDelta = cats.map(c => ({ ...c, delta: c.thisMonthCents - c.lastMonthCents }));
const topGainer = withDelta.reduce((best, c) => c.delta < best.delta ? c : best); // most improved
const topRiser = withDelta.reduce((best, c) => c.delta > best.delta ? c : best); // biggest increase
```

Only show if `topGainer.delta < 0` (actually dropped) and `topRiser.delta > 0` (actually rose).

**Rendered markup (muted italic, small font):**
```
✦ [Dining Out] dropped $142 — the biggest improvement this month.
  [Groceries] rose by $89.
```

Uses `money(Math.abs(topGainer.delta))` and `money(topRiser.delta)` with 0 decimals.

### §9b — Apply what-if slider (`Goals.tsx`)

**Change:** In the existing what-if panel, the "Reset" button area gains an "Apply" button.

**Visibility:** "Apply" button only rendered when `extra > 0` and `newMonths !== null`.

**Action:** 
```typescript
const updateMonthly = useUpdateGoalMonthly();

const handleApply = async () => {
  const newMonthly = scenarioGoal.monthlyCents + extra * 100;
  try {
    await updateMonthly.mutateAsync({ id: scenarioGoal.id, monthlyCents: newMonthly });
    toast.success(`Applied +${fmt(extra * 100)}/mo to ${scenarioGoal.name}`, {
      description: newMonths! > 0 ? `ETA now ${etaLabel(newMonths!)}` : "Goal reached this month!",
    });
    setExtra(0);
  } catch {
    toast.error("Failed to apply change");
  }
};
```

**Button layout:** `[Reset]  [Apply +$500/mo →]` (Apply in primary style, disabled while pending).

Replace the local `fmt()` in Goals.tsx with the shared `money()` from `../utils/format`. (Goals.tsx still has its own `fmt` — this is a good time to clean it up.)

### §9c — Sinking funds section (`Goals.tsx`)

**Position:** Below the main goal list grid, before the what-if panel. Eyebrow: "Sinking funds · due within a year".

**Filter:**
```typescript
function daysUntil(dateStr: string): number {
  return (new Date(dateStr).getTime() - Date.now()) / 86400000;
}
const sinkingFunds = goals.filter(
  (g) => g.goalType === "save-by-date" && g.targetDate && daysUntil(g.targetDate) <= 365 && daysUntil(g.targetDate) > 0
);
```

**Layout:** 2-column grid (`gridTemplateColumns: "1fr 1fr"`, gap 12).

Each compact card (smaller padding than GoalCard):
- Goal name (bold, 14px)
- Target date chip (e.g., "Dec 2026")
- Progress bar (currentCents / targetCents, color = goal.color)
- `money(goal.targetCents - goal.currentCents)` remaining + pct%
- Left border accent: `borderLeft: 3px solid ${goal.color}`

Cards are read-only (not clickable). If a sinking fund also appears in the main list above, it shows in both places — that's acceptable and provides a useful "spotlight" view.

**Not shown when:** `sinkingFunds.length === 0`.

### §11b — Rules manual new-rule builder (`Rules.tsx`)

**Trigger:** "New rule" button in the screen header (right side).

**State:** `showNewRule: boolean` in the `Rules` component.

**Form:** Inline card that appears at the top of the rules list (above active rules):

```
Pattern:   [__________]  e.g. %starbucks%
Category:  [dropdown ▾]  (all non-archived categories from useCategoriesWithSpending)
Preview:   when merchant contains "starbucks" → [Dining Out]
           [Create rule]  [Cancel]
```

**Validation:** Both fields required. Pattern must be non-empty. Category must be selected.

**Category dropdown:** Uses `useCategoriesWithSpending()` (already called in the Rules screen indirectly via proposals). Build a local sorted list of `{ id, label }` from categories. Sort alphabetically.

**Submit:**
```typescript
const createRule = useCreateRule(); // new hook wrapping commands.createRule

await createRule.mutateAsync({ pattern, categoryId });
toast.success("Rule created");
setShowNewRule(false);
```

**New hook** added to `ui/src/api/hooks/transactions.ts`:
```typescript
export function useCreateRule() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ pattern, categoryId }: { pattern: string; categoryId: string }) =>
      commands.createRule(pattern, categoryId).then(unwrap),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["rules"] }),
  });
}
```

**Pattern normalization:** If user doesn't include `%`, auto-wrap: `pattern.includes("%") ? pattern : \`%${pattern}%\``.

### §12a, §12b, §12c — Settings screen (`Settings.tsx`)

Two new sections appended after the existing AI Provider section:

#### Appearance section

```
h2: "Appearance"

Theme       [Light]  [Dark]           ← .toolbar buttons backed by useTweaks().setTheme
Density     [Cozy]   [Compact]        ← useTweaks().setDensity
Accent      ● ● ● ● ● ●              ← 6 swatches (ACCENTS from tweaks.ts), active = ring
Currency    [USD ▾]                   ← <select> with major currencies, backed by useSetCurrency
```

Currency `<select>` options: USD, EUR, GBP, CAD, AUD, JPY, CHF, NZD, SGD, HKD.

On currency change: call `useSetCurrency().mutate(value)` — this both saves to backend KV and updates `useTweaks` localStorage.

#### Data export section

```
h2: "Export data"
p: "Download your complete data as JSON or a transaction CSV."

[Export as JSON]  [Export as CSV]
```

Each button calls the respective export mutation. While pending: button shows "Exporting…" and is disabled. On success: `toast.success("File saved")`. On error: `toast.error("Export failed — " + err.message)`.

### §13a — Agent operator panel (`Insights.tsx`)

New `AgentStatusBar` component at the top of Insights (before insight cards), implemented inline in `Insights.tsx`.

**Layout:**
```
[● pulse] Agent · running locally          [← cycling ticker →]  [Re-run scan]
```

**Ticker:** 6 hard-coded messages at module scope:
```typescript
const TICKERS = [
  "Watching: account balances · stable",
  "Reviewing: transaction categories",
  "Monitoring: recurring subscriptions",
  "Analyzing: spending patterns",
  "Tracking: goal progress",
  "Checking: rule coverage",
];
```

Cycles every 2400ms with `setInterval`. Cleanup on unmount via `clearInterval`. Crossfade effect: `opacity` transitions via `transition: opacity 0.3s`.

**Re-run scan button:**
```typescript
const triggerCategorize = useTriggerCategorize(); // already exists in agent.ts hooks

<button
  className="btn sm ghost"
  disabled={triggerCategorize.isPending}
  onClick={async () => {
    try {
      await triggerCategorize.mutateAsync();
      toast.success("Scan complete");
    } catch {
      toast.error("Scan failed");
    }
  }}
>
  {triggerCategorize.isPending ? "Scanning…" : "Re-run scan"}
</button>
```

**Pulse dot:** CSS `@keyframes pulse { 0%,100% { opacity:1 } 50% { opacity:0.3 } }` with `animation: pulse 2s infinite`. Uses existing `var(--accent)` color.

### §14a — Command palette "Ask the agent" mode (`CommandPalette.tsx`)

**State additions to `CommandPalette`:**
```typescript
type PaletteMode = "list" | "answer";
const [mode, setMode] = useState<PaletteMode>("list");
const [activeQ, setActiveQ] = useState<CannedQuestion | null>(null);
```

**Data loading** — fired when `open === true`:
```typescript
const { data: totals } = useQuery({ queryKey: ["month-totals"], queryFn: ..., enabled: open });
const { data: cats = [] } = useQuery({ queryKey: ["categories"], queryFn: ..., enabled: open });
const netWorth = useNetWorth(); // derived from cached accounts+assets+liabilities, no extra fetch
```

**`CannedQuestion` type:**
```typescript
interface CannedQuestion {
  label: string;
  prose: string;
  kind: "bigNumber" | "compareBars" | "progress";
  vizData: BigNumberData | CompareBarsData | ProgressData;
  actionLabel?: string;
  actionPath?: string;
}
```

**5 canned questions** (derived once both `totals` and `cats` are loaded):
```typescript
const questions = useMemo<CannedQuestion[]>(() => {
  if (!totals || cats.length === 0) return [];
  const topCat = [...cats].sort((a, b) => b.thisMonthCents - a.thisMonthCents)[0];
  const overBudget = cats.filter(c => c.budgetCents > 0)
    .sort((a, b) => (b.thisMonthCents / b.budgetCents) - (a.thisMonthCents / a.budgetCents))[0];
  const dayOfMonth = new Date().getDate();
  const avgDailyBurn = totals.expenseCents / dayOfMonth;
  // netWorth is from useNetWorth() — derived from cached queries, no extra fetch
  const runwayDays = avgDailyBurn > 0 ? Math.round(netWorth / avgDailyBurn) : null;
  return [
    {
      label: "What's my top spending category this month?",
      prose: topCat
        ? `Your biggest expense category is ${topCat.categoryLabel} at ${money(topCat.thisMonthCents)}.`
        : "No spending data yet.",
      kind: "bigNumber",
      vizData: { value: money(topCat?.thisMonthCents ?? 0), label: topCat?.categoryLabel ?? "—" },
      actionLabel: "Open Categories →",
      actionPath: "/categories",
    },
    {
      label: "How does my spending compare to last month?",
      prose: `This month: ${money(totals.expenseCents)}. Last month: ${money(cats.reduce((s,c) => s+c.lastMonthCents, 0))}.`,
      kind: "compareBars",
      vizData: { thisMonth: totals.expenseCents, lastMonth: cats.reduce((s,c) => s+c.lastMonthCents, 0) },
      actionLabel: "Open Reports →",
      actionPath: "/reports",
    },
    {
      label: "What's my current savings rate?",
      prose: `You're keeping ${totals.savingsRatePct}% of your income this month.`,
      kind: "bigNumber",
      vizData: { value: `${totals.savingsRatePct}%`, label: "of income kept" },
      actionLabel: "Open Today →",
      actionPath: "/",
    },
    {
      label: "Which category am I closest to maxing out?",
      prose: overBudget
        ? `${overBudget.label} is at ${Math.round((overBudget.thisMonthCents / overBudget.budgetCents) * 100)}% of budget.`
        : "No budgets set yet.",
      kind: "progress",
      vizData: overBudget
        ? { label: overBudget.label, pct: Math.min(120, (overBudget.thisMonthCents / overBudget.budgetCents) * 100) }
        : { label: "—", pct: 0 },
      actionLabel: "Open Budget →",
      actionPath: "/budget",
    },
    {
      label: "What's my financial runway?",
      prose: runwayDays !== null
        ? `At your current burn rate, you have ${runwayDays} days of runway.`
        : "Not enough spending data to estimate runway.",
      kind: "bigNumber",
      vizData: { value: runwayDays !== null ? `${runwayDays}` : "—", label: "days runway" },
      actionLabel: "Open Accounts →",
      actionPath: "/accounts",
    },
  ];
}, [totals, cats]);
```

**List mode rendering** — New "Ask the agent" section above "Jump to" in the palette list:
```jsx
{questions.length > 0 && (
  <>
    <div className="cmdk-section">Ask the agent</div>
    {questions.map((q, i) => (
      <div
        key={i}
        className={`cmdk-item${sel === askStartIdx + i ? " sel" : ""}`}
        onClick={() => { setActiveQ(q); setMode("answer"); }}
      >
        <I.Sparkle className="ico" />
        <span>{q.label}</span>
      </div>
    ))}
  </>
)}
```

**Answer mode rendering** — replaces the normal palette body:
```jsx
{mode === "answer" && activeQ && (
  <div className="cmdk-answer" style={{ padding: "20px 24px", maxWidth: "min(760px, 94vw)" }}>
    <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 12 }}>
      <div className="eyebrow"><I.Sparkle /> {activeQ.label}</div>
      <button className="btn sm ghost" onClick={() => setMode("list")}>← Back</button>
    </div>
    <p style={{ marginBottom: 16 }}>{activeQ.prose}</p>
    <AskViz data={activeQ} />
    {activeQ.actionLabel && (
      <button className="btn primary" onClick={() => { navigate(activeQ.actionPath!); onClose(); }}>
        {activeQ.actionLabel}
      </button>
    )}
  </div>
)}
```

**`AskViz` component** (inline in CommandPalette.tsx):
- `bigNumber`: large centered `value` + `label` below in muted
- `compareBars`: two labeled divs, widths proportional, this-month vs last-month
- `progress`: labeled progress bar, color red when pct > 100

**Width expansion:** In answer mode, apply `maxWidth: "min(760px, 94vw)"` on `.cmdk` container via conditional inline style.

**Keyboard navigation:** In answer mode, Escape returns to list mode (not close), Enter on Back button returns to list.

### §14b — Additional palette actions (`CommandPalette.tsx`)

Add one new item to `ACT_ITEMS`:
```typescript
{ kind: "act", label: "Run a what-if scenario", Icon: I.Bolt, path: "/scenarios" },
```

The `handleItem` function already handles `item.path` navigation, so no other changes needed.

---

## Implementation order

Tasks should be executed in this order (each is independent after the backend+hooks are done):

| Task | File(s) changed | Backend? | New file? |
|------|----------------|----------|-----------|
| 1. Backend commands | `goals.rs`, `budget.rs`, `settings.rs` (new), `lib.rs` | ✅ | settings.rs |
| 2. Hooks | `recurring.ts` (new), `budget.ts`, `settings.ts` (new), `tweaks.ts` | — | recurring.ts, settings.ts |
| 3. §3b Smart Sweep | `Today.tsx` | — | — |
| 4. §3c Recurring chips | `Today.tsx` | — | — |
| 5. §3d Runway stat | `Today.tsx` | — | — |
| 6. §6c AI insight | `Categories.tsx` | — | — |
| 7. §9b Apply what-if | `Goals.tsx` | — | — |
| 8. §9c Sinking funds | `Goals.tsx` | — | — |
| 9. §11b Rules builder | `Rules.tsx` | — | — |
| 10. §12a+b+c Settings | `Settings.tsx`, `tweaks.ts` | — | — |
| 11. §13a Agent panel | `Insights.tsx` | — | — |
| 12. §14a Ask the agent | `CommandPalette.tsx` | — | — |
| 13. §14b Palette actions | `CommandPalette.tsx` | — | — |

Tasks 3–5 all touch `Today.tsx` — plan and implement them together as one task to avoid merge conflicts.  
Tasks 7–8 both touch `Goals.tsx` — same.  
Tasks 12–13 both touch `CommandPalette.tsx` — implement §14b as part of §14a's task.

**Effective task count: 9 tasks** (backend, hooks, Today[§3b+§3c+§3d], Categories, Goals[§9b+§9c], Rules, Settings[§12a+§12b+§12c], Insights, CommandPalette[§14a+§14b]).

---

## Testing requirements

Each task must include tests. Minimum coverage:

| Task | Required tests |
|------|----------------|
| Backend | Rust unit test: `update_goal_monthly` persists correctly |
| Hooks | Hook tests are covered by component tests |
| Today §3b | Test: Smart Sweep card shown when netCents > 5000, hidden when dismissed |
| Today §3c | Test: items due within 7 days shown, past items not shown |
| Today §3d | Test: runway stat shown with correct value; "—" when no burn data |
| Categories §6c | Test: insight sentence shown when last-month data exists; not shown when all lastMonthCents = 0 |
| Goals §9b | Test: Apply button calls updateGoalMonthly with correct monthlyCents |
| Goals §9c | Test: sinking fund cards shown for save-by-date goals within 1 year |
| Rules §11b | Test: form appears on button click; createRule called with auto-wrapped pattern |
| Settings | Test: theme/density/accent controls render and fire useTweaks setters; export buttons render |
| Insights §13a | Test: status bar renders; Re-run scan button calls triggerCategorize |
| CommandPalette §14a | Test: ask section shown when data loaded; answer mode renders for each question |

**Green bar target:** All existing 72 tests pass + new tests for each feature. TypeScript: 0 errors.

---

## Field-casing note

- `Transaction` type: **snake_case** (`is_reimbursable`, `amount_cents`)
- All Wave B types (`GoalDto`, `RecurringItem`, etc.): **camelCase** (via `serde(rename_all = "camelCase")`)
- Always check `bindings.ts` when accessing fields on a new type

---

## CSS / design conventions

- Design tokens only: `var(--ink)`, `var(--ink-mute)`, `var(--ink-faint)`, `var(--line)`, `var(--accent)`, `var(--negative)`, `var(--surface-2)` etc.
- Utility classes: `.card`, `.chip`, `.btn`, `.eyebrow`, `.toolbar`, `.stub`, `.muted`, `.num`, `.money`
- Pulse animation for the agent dot: `@keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.3} }` — add to `app.css` if not already present, or inline via `style` with `animation`
- Icons: import from `ui/src/components/Icons.tsx`
- Money formatting: `money(cents, opts?)` from `ui/src/utils/format.ts` — never use local `fmt()` copies

---

## Out of scope for Wave B

The following items remain for future waves:
- §2 Plan Next Month wizard (complex 6-step modal + backend)
- §5c Transaction CSV export (needs its own backend command)
- §4c Account CSV export
- §7c Budget 5-month history strip (new backend query)
- §8b Recurring price-history chip
- §10a–e Reports dashboard improvements
- §11c Agent 24h activity log (new backend command)
- §15 items (all done)
