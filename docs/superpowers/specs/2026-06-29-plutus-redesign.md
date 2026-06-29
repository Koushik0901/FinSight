# FinSight UI Redesign — Plutus Design Language
**Date:** 2026-06-29  
**Status:** Approved  
**Scope:** Full visual redesign + navigation restructure matching the Plutus reference design

---

## 1. Goals

- Reduce navigation from 18 routes to exactly 12 (10 main + 2 workshop) matching the Plutus sidebar
- Every screen must have an eyebrow label + large hero heading (Plutus pattern)
- Visual language: pure dark surfaces, large data typography, inline smart chips, sparkline rows
- FinSight-specific features (Copilot, Journey, Inbox, Import Review, Recipes) fold into the correct screens — none become orphan pages

---

## 2. Navigation Restructure

### Final sidebar (exact Plutus match)

**MAIN**
| Route | Label | Badge |
|---|---|---|
| `/` | Today | — |
| `/insights` | Insights | live dot when agent ran recently |
| `/accounts` | Accounts | total account count |
| `/transactions` | Transactions | monthly count |
| `/budget` | Budget | — |
| `/categories` | Categories | — |
| `/recurring` | Recurring | — |
| `/goals` | Goals | active goal count |
| `/scenarios` | Scenarios | — |
| `/reports` | Reports | — |

**WORKSHOP**
| Route | Label |
|---|---|
| `/rules` | Rules & agents |
| `/settings` | Settings |

**Footer (non-nav)**
- "Run setup again" ghost button
- "Local-only · synced Xm ago" trust line

### Pages removed from nav and their fate

| Removed route | Disposition |
|---|---|
| `/inbox` | Content merged into Today (notifications feed becomes the Smart Sweep + Morning Briefing section) |
| `/copilot` | Removed as a page. CopilotQuickAsk floating button persists on Today, Transactions, Budget |
| `/journey` | Journey milestones and health score fold into Today (hero stats) and Reports (Wealth & FIRE tab) |
| `/import-review` | Converted to a Drawer triggered by an "Import review" badge/button in the Accounts screen |
| `/recipes` | Recipes tab added inside `/rules` (Rules & agents screen gains two tabs: Rules · Recipes) |

---

## 3. Visual Design System — Plutus Language

### Existing tokens (already correct — no changes needed)
- `--bg: #08080B` — near-black, matches Plutus
- `--accent: #C9F950` — lime green, matches Plutus
- `--ink: #F4F4F7` — primary text
- `--ink-mute` / `--ink-faint` — muted text hierarchy

### New CSS patterns to add (in `app.css`)

#### Page hero pattern (THE defining Plutus element)
Every screen uses this structure:
```
.page-eyebrow   → small-caps muted label e.g. "TRANSACTIONS · MAY 2026 · 1,247 INDEXED"
.page-hero      → giant H1, ~52–64px, white, descriptive copy
.page-toolbar   → top-right action buttons row (existing .toolbar can extend)
```

```css
.page-eyebrow {
  font: 600 10px/1 var(--sans);
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--ink-faint);
  display: flex; align-items: center; gap: 8px;
}
.page-eyebrow .live-dot {
  width: 6px; height: 6px; border-radius: 50%;
  background: var(--accent); flex-shrink: 0;
}
.page-hero {
  font: 700 56px/1.05 var(--sans);
  letter-spacing: -0.03em;
  color: var(--ink);
  margin: 8px 0 0;
}
/* responsive */
@media (max-width: 1100px) { .page-hero { font-size: 42px; } }
```

#### Stat card grid (used on Today, Accounts, Reports)
```css
.stat-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 2px; /* tight gap between cards */
}
.stat-card {
  background: var(--surface);
  padding: 20px 24px;
  border-radius: var(--radius);
}
.stat-card.accent { background: color-mix(in srgb, var(--accent) 8%, var(--surface)); }
.stat-card .label { /* eyebrow on card */ font: 600 10px/1 var(--sans); letter-spacing: 0.1em; text-transform: uppercase; color: var(--ink-faint); }
.stat-card .value { font: 700 32px/1.1 var(--sans); letter-spacing: -0.02em; margin-top: 6px; }
.stat-card .delta { font-size: 12px; margin-top: 4px; display: inline-flex; align-items: center; gap: 4px; padding: 2px 8px; border-radius: var(--radius-pill); }
.stat-card .delta.pos { background: var(--positive-2); color: var(--positive); }
.stat-card .delta.neg { background: var(--negative-2); color: var(--negative); }
```

#### Inline smart chips (used on transaction rows)
```css
.chip-inline {
  display: inline-flex; align-items: center; gap: 4px;
  padding: 1px 7px; border-radius: var(--radius-pill);
  font-size: 11px; font-weight: 500; line-height: 18px;
}
.chip-inline.ai    { background: var(--accent-2); color: var(--accent); }
.chip-inline.warn  { background: var(--warning-2); color: var(--warning); }
.chip-inline.tag   { background: var(--elevated); color: var(--ink-mute); }
```

#### Filter tab bar (used on Transactions, Goals, Reports, Budget)
```css
.filter-tabs {
  display: flex; align-items: center; gap: 2px;
  background: var(--surface); border-radius: var(--radius);
  padding: 3px;
}
.filter-tab {
  padding: 5px 12px; border-radius: 7px;
  font: 500 13px/1 var(--sans); color: var(--ink-mute);
  border: none; background: none; cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.filter-tab.active { background: var(--elevated); color: var(--ink); }
.filter-tab .count { 
  font-size: 11px; color: var(--ink-faint); margin-left: 4px;
}
.filter-tab.active .count { color: var(--ink-mute); }
```

#### Merchant avatar (used on transaction rows)
```css
.merchant-avatar {
  width: 32px; height: 32px; border-radius: 8px;
  display: flex; align-items: center; justify-content: center;
  font: 700 12px var(--sans); flex-shrink: 0;
  color: #fff;
}
```

#### Sparkline cell (used on Accounts rows)
Tiny inline SVG line chart, 60×24px, rendered per account balance history.

---

## 4. Screen Specifications

### 4.1 Today (`/`)

**Eyebrow:** `• TUESDAY · {date}`  
**Top-right pills:** `🔒 Local-only` · `• Agent · ran {N}m ago`  
**Hero:** `NET WORTH` (eyebrow above giant number)  
**Big number:** current net worth in giant type (56–72px)  
**Delta row:** `+$X,XXX in the last 30 days · You're tracking X% below {month} spending.`

**Section: Net worth chart**
- Full-width area chart with lime green line and subtle fill
- Time range tabs: `1M  3M  6M  1Y  All`
- Shows last known balance point on hover

**Section: 4-column stat grid**
| LIQUID | INVESTED | CREDIT | RUNWAY · AT CURRENT BURN |
- Runway card: accent-colored, shows days + `$X,XXX/mo · ends {date}`

**Section: Two-column smart cards (2-up)**
- Left: `• MORNING BRIEFING · 60 SECONDS` — 2–3 sentence AI narrative of the week's highlights (from agent, replaces Inbox)
- Right: `SMART SWEEP · SUGGESTED` — one actionable suggestion (move money to goal / pay down credit / etc.)

**FinSight additions folded in:**
- Health score: shown as a small chip next to the net worth delta row: `A · 91 pts`
- Uncelebrated milestones: modal-overlay celebration card, shown once then dismissed (existing logic)
- CopilotQuickAsk: floating button bottom-right

**Removed from Today:**
- AgentActivityFeed (was too noisy — feeds move to Insights)
- Monthly review CTA (moves to Reports)
- SavingsSparkline section (savings rate visible on Reports)

---

### 4.2 Insights (`/insights`)

**Eyebrow:** `• INSIGHTS · {month year}`  
**Dot:** live green dot when agent has fresh output  
**Hero:** `What the numbers are saying.`

**Section: Agent activity feed** (moved here from Today)
- Each insight card: icon + headline + body paragraph + optional action button
- Categories: Anomaly | Trend | Opportunity | Warning

**Section: Spending patterns**
- Bar chart: category spend last 6 months
- Highlight the biggest mover month-over-month

**FinSight additions:**
- Copilot session list replaces the old Copilot page. Show last 5 sessions with "Ask a question" CTA that opens CopilotQuickAsk drawer.

---

### 4.3 Accounts (`/accounts`)

**Eyebrow:** `• ACCOUNTS · {N} CONNECTED · {M} MANUAL`  
**Top-right:** `Connect bank` button + `+ Add manual asset` button  
**Hero:** `Everything in one place.`

**Section: 4-column stat grid**
| ASSETS · CONNECTED | ASSETS · MANUAL | LIABILITIES | NET WORTH (accent card) |

**Section: Two-column layout**
- Left column (account list):
  - Grouped by person / joint
  - Each row: colored dot | name | bank hint | sparkline (60×24) | balance (right-aligned, money class)
  - Group header: person name + group total (muted)
  - Manual assets section below with `Update monthly` button
  - Import review badge: `• {N} pending` orange dot → opens ImportReview Drawer on click
- Right column (account detail pane):
  - Shown when account selected; hidden on empty state
  - Header: bank · last4 · account name · balance (large)
  - Sub-row: `☆ Auto-synced` + `+$X,XXX · 30d` delta chip
  - Balance sparkline (large, 7–14 day)
  - `RECENT ACTIVITY` table with Filter + Export buttons
  - Rows: date | merchant avatar | merchant name | category dot + label | amount

---

### 4.4 Transactions (`/transactions`)

**Eyebrow:** `• TRANSACTIONS · {MONTH YEAR} · {N,NNN} INDEXED`  
**Top-right:** `↓ Export` + `+ Add manual` buttons  
**Hero:** `Every line of activity, searchable.`

**Search bar:** full-width, `Search by merchant, note, amount, or category…`

**Filter tab bar:** `All {N}` | `Needs review {N}` | `Split {N}` | `Reimbursable {N}` | `Anomalies {N}` | `Trips {N}` (show tab only if count > 0, except All)

**Date range + account filter row** (keep existing functionality, restyled as inline toolbar below tab bar)

**Table columns:** DATE | MERCHANT | CATEGORY | ACCOUNT | AMOUNT

**Row detail:**
- DATE: day/month stacked in small muted text
- MERCHANT: avatar (32×32, colored initials) | merchant name bold | note in muted below | AI chip if auto-categorized
- CATEGORY: colored dot + label  
- ACCOUNT: colored dot + account name
- AMOUNT: right-aligned, green for income, white for expense; `money` class

**Inline smart chips on rows** (Plutus exact):
- `✦ Categorized via merchant similarity` (accent chip)
- `✦ Price changed from $X.XX` (warning chip)
- `reimbursable` (tag chip)
- `split N` (tag chip)
- `N.Nx your 12-month average` (warning chip)

**Bulk action bar** (appears when rows checked): `Categorize · Ignore · Export selected`

---

### 4.5 Budget (`/budget`)

**Eyebrow:** `• BUDGET · {MONTH YEAR} · DAY {N} OF {M}`  
**Top-right:** `✦ Plan next month` (accent button) + `Envelope | Tracking` tab toggle  
**Hero:** `Where the plan stands today.`

**Month progress card:**
- Left: `MONTH PROGRESS` eyebrow + large `$X,XXX left to spend`  
- Progress bar: lime fill, gray track, `{N}% through {month} · {N}% spent · {N} days left`
- Right stats grid: BUDGETED | SPENT SO FAR | PROJECTED EOM (with delta chip)
- Below: AI insight sentence: `On pace to end {month} at about $X,XXX — $XXX under budget…`

**Unassigned banner:**
- `✦ $X,XXX of $X,XXX income · $X,XXX assigned`
- Right: `Assign to a goal` | `Park in House Fund` buttons

**Section: NEEDS A GLANCE · {N}**
- Sub-heading: `Just these — the rest is fine.`
- 2-column grid of envelope cards:
  - Card border: orange (tight) or red (over)
  - Header: icon + name + status chip (`Tight`, `Over by $X`)
  - Big number: `$X,XXX left to spend` or `$XX over budget`
  - Progress bar: lime (on track), orange (tight), red (over)
  - Footer row: `spent $X,XXX of $X,XXX` + rollover delta
  - Action button: `About $X per day left to stay under.` / `Cover $X from another envelope`

**Section: All envelopes** (Envelope tab) — full table of all categories with progress bars

**Section: Tracking view** — time-series chart of budget adherence (existing Tracking tab content)

---

### 4.6 Categories (`/categories`)

**Eyebrow:** `• CATEGORIES · {MONTH YEAR}`  
**Top-right:** period tabs `This month | vs. average | Year`  
**Hero:** `Where the money is going.`

**Summary card:**
- `THIS MONTH` eyebrow + total spend in large type (40px)
- Right: `vs. {prev month}` delta
- Below: colored horizontal bar chart — each category a colored segment, proportional to spend
- AI sentence: `{Category} dropped $X,XXX — the biggest move. {Category} rose by $XX.`
- Sub-text: `Budget set by you · agent suggests adjustments quarterly`

**Section: All categories — table**
Columns: CATEGORY (icon + name) | PACE (mini bar + %) | THIS MONTH | {PREV MONTH} | BUDGET | TRANSACTIONS

- PACE bar: lime if under, orange if 80–99%, red if over 100%
- Clicking a row opens TransactionDrawer filtered to that category

**FinSight addition:** `spending_type` badge (Need / Want / Saving / Investment) as a subtle chip on each row, hidden unless hovered. Allocation donut remains as a secondary view toggle.

---

### 4.7 Recurring (`/recurring`)

**Eyebrow:** `• RECURRING · {N} TRACKED`  
**Hero:** `What shows up every month.`

**Section: Summary row** — Total committed monthly: `$X,XXX/mo` | Next 7 days: `$XXX` | Annual: `$X,XXX`

**Section: Table**
Columns: MERCHANT | FREQUENCY | NEXT DATE | AMOUNT | CATEGORY | STATUS

- Status chip: `Active` (muted) | `Due soon` (warning) | `Missed` (negative) | `New` (accent)
- Rows sorted by next payment date

**FinSight addition:** Agent-detected anomalies inline (price change chip on rows where amount drifted).

---

### 4.8 Goals (`/goals`)

**Eyebrow:** `• GOALS · {N} ACTIVE`  
**Top-right:** `+ New goal` button  
**Hero:** `Things you're moving toward.`

**Sub-copy:** `A goal is a horizon line on your future runway. The agent moves money toward each on the cadence you set, and shows you when reality drifts from the plan.`

**Filter tab bar:** `All {N}` | `Save by date {N}` | `Build balance {N}` | `Debt payoff {N}` | `Spending cap {N}`

**Goal cards:**
- Header row: type badge (e.g. `Save by date`) | owner badge (`Joint` / `Mira` / `Adam`) | status badge (`• on track` green / `• ahead` / `• needs attention` red)
- Name (large, 20px bold) + sub-line: `Auto-moves $X/mo · ETA {month year}`
- Right column: `PROGRESS` label + progress bar (lime) + `$X,XXX of $X,XXX`
- Far right: percentage `76%` in large muted type + `Pause` | `Adjust` buttons

**Tabs within Goals:**
- `All goals` — list above
- `Debt payoff` tab — full debt snowball / avalanche calculator (existing DebtPayoff component, restyled to match card pattern)

---

### 4.9 Scenarios (`/scenarios`)

**Eyebrow:** `• SCENARIOS`  
**Hero:** `What if you changed the plan.`

**Section: Scenario cards** — existing Scenarios screen content, restyled to card pattern with hero heading

---

### 4.10 Reports (`/reports`)

**Eyebrow:** `• REPORTS · {MONTH YEAR} · BUILD YOUR OWN VIEW`  
**Top-right:** period tabs `May | Quarter | Year | All-time` + `↓ Export` + `✎ Customize` buttons  
**Hero:** `How money is moving.`

**Sub-tab bar:** `Monthly overview` | `Wealth & FIRE` | `Spending deep dive`

**Monthly overview tab:**
- 4-column stat grid: Savings rate | Net worth | Spent this month | Runway
- Each with delta chip vs. prior period
- Flow chart (Sankey-style): income sources → total → spending categories → saved; labels on each block
- Two charts below: `Spending · {year} vs {year-1}` line chart | `By category · this month` donut + legend

**Wealth & FIRE tab:**
- FinSight's compound growth projector (10/20/30-year at 7%)
- Net worth trend + FIRE number progress
- Journey milestones (from /journey): 7-step milestone timeline folded in here as a progress indicator

**Spending deep dive tab:**
- Category breakdown with month-over-month comparison table
- Budget adherence history chart (12 months)
- Monthly review records (list of saved monthly reviews)

---

### 4.11 Rules & agents (`/rules`)

**Eyebrow:** `• RULES & AGENTS · WORKSHOP`  
**Hero:** `Teach it how you think.`

**Two-tab layout:**
- `Rules` tab — existing Rules screen content, restyled
- `Recipes` tab — existing Recipes screen content, restyled

Both tabs share the same hero. Tabs render as the `filter-tabs` component.

---

### 4.12 Settings (`/settings`)

**Eyebrow:** `• SETTINGS`  
**Hero:** `Make it yours.`

**Left sub-nav + right content pane** (exact Plutus layout):
- Sub-nav items: Profile | Privacy & data | Agent | Appearance | Household | Connections | Developer API | Notifications | Keyboard | About
- Content pane renders the relevant section

---

## 5. Sidebar Visual Spec

The sidebar already has the right structure. Visual updates needed:

1. **Nav section header:** `WORKSHOP` — all caps, 10px, var(--ink-faint), letter-spacing 0.12em, margin 16px 0 4px 12px
2. **Nav item active state:** lime left border (2px) + slightly lighter background
3. **Nav item badge:** right-aligned pill showing count — `background: var(--elevated); color: var(--ink-mute); font-size: 11px`
4. **Live dot on Insights:** 6px lime circle, CSS animation `pulse` when data is fresh
5. **User avatar:** 32px circle with initials (existing `who` area — keep as-is)
6. **Command palette bar:** keep `⌘K` hint right-aligned
7. **Footer:** `Run setup again` + trust line — keep existing

---

## 6. Implementation Decomposition

This spec decomposes into three implementation phases:

### Phase 1 — Navigation restructure + CSS foundations
- Remove `/inbox`, `/copilot`, `/journey`, `/import-review`, `/recipes` routes from App.tsx and Sidebar
- Add ImportReview Drawer to Accounts screen (triggered by badge)
- Merge Recipes tab into Rules screen
- Add `.page-eyebrow`, `.page-hero`, `.stat-grid`, `.stat-card`, `.filter-tabs`, `.chip-inline`, `.merchant-avatar` CSS patterns to `app.css`
- Update Sidebar: remove deleted nav items, clean up order, add active border style

### Phase 2 — Core screens (Today, Transactions, Budget, Categories)
Full visual rebuild of the 4 most-used screens to exactly match Plutus screenshots.

### Phase 3 — Supporting screens (Accounts, Recurring, Goals, Scenarios, Reports, Rules & agents, Settings)
Full visual rebuild of remaining 8 screens.

---

## 7. Out of Scope

- Backend changes — all data is already available via existing hooks
- New Tauri commands — not needed for this redesign
- Light theme parity — dark theme first; light theme tokens already exist and will inherit improvements
- Onboarding screen — separate flow, not touched
- Mobile/responsive — Tauri desktop app, minimum 1024px wide assumed
