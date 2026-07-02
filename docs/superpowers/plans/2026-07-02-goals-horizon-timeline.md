# Goals Horizon Timeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Horizon" timeline section to the Goals screen — a single shared timeline showing every eligible goal's projected finish date as a colored marker with a progress-filled track, matching the Claude Design mockup.

**Architecture:** A pure helper `buildHorizonRows(goals)` (new, exported from `ui/src/screens/Goals.tsx`, co-located with the existing `monthsToGoal`/`etaLabel` functions it calls) filters out spending-cap goals and goals with no finite ETA, computes each remaining goal's months-to-completion, progress percentage, and whether it's behind its own target date, sorts soonest-first, and sizes the timeline window dynamically. A new `GoalsHorizon` component renders that data as a card with a month axis, a "today" marker, and one row per goal.

**Tech Stack:** React + TypeScript, Vitest + `@testing-library/react`, no backend/DB changes.

**Reference:** Design doc at `docs/superpowers/specs/2026-07-02-goals-horizon-timeline-design.md`.

---

### Task 1: Add the `buildHorizonRows` pure helper

**Files:**
- Modify: `ui/src/screens/Goals.tsx`
- Test: `ui/src/screens/Goals.test.tsx`

- [ ] **Step 1: Write the failing tests**

Add this new `describe` block to `ui/src/screens/Goals.test.tsx`, right after the existing `describe("Goals — eyebrow casing", ...)` block (before `describe("Goals — pause/resume", ...)`). This tests `buildHorizonRows` directly as a pure function — it does not render anything, so the existing `vi.mock` setup in the file doesn't affect it:

```ts
import { buildHorizonRows } from "./Goals";

function future(monthsFromNow: number): string {
  const d = new Date();
  d.setMonth(d.getMonth() + monthsFromNow);
  return d.toISOString().slice(0, 10);
}

describe("Goals — buildHorizonRows", () => {
  const baseGoal = {
    id: "x", name: "X", color: "#C9F950", notes: null, purpose: null,
    sortOrder: 0, createdAt: "2026-01-01", liabilityId: null, accountId: null, targetDate: null,
  };

  it("excludes spending-cap goals even when they would have a finite ETA", () => {
    const spendingCap = { ...baseGoal, id: "sc1", goalType: "spending-cap", targetCents: 40000, currentCents: 10000, monthlyCents: 10000 };
    const { rows } = buildHorizonRows([spendingCap]);
    expect(rows).toHaveLength(0);
  });

  it("excludes goals with no monthly contribution and incomplete progress (infinite ETA)", () => {
    const stalled = { ...baseGoal, id: "st1", goalType: "save-by-date", targetCents: 500000, currentCents: 100000, monthlyCents: 0 };
    const { rows } = buildHorizonRows([stalled]);
    expect(rows).toHaveLength(0);
  });

  it("includes an already-complete goal at months: 0 and xPercent: 0", () => {
    const done = { ...baseGoal, id: "d1", goalType: "save-by-date", targetCents: 100000, currentCents: 150000, monthlyCents: 5000 };
    const { rows } = buildHorizonRows([done]);
    expect(rows).toHaveLength(1);
    expect(rows[0]!.months).toBe(0);
    expect(rows[0]!.xPercent).toBe(0);
  });

  it("clamps pct to 100 when currentCents exceeds targetCents", () => {
    const over = { ...baseGoal, id: "o1", goalType: "build-balance", targetCents: 100000, currentCents: 150000, monthlyCents: 5000 };
    const { rows } = buildHorizonRows([over]);
    expect(rows[0]!.pct).toBe(100);
  });

  it("sizes the window to a floor of 6 months when all goals are near-term", () => {
    const near = { ...baseGoal, id: "n1", goalType: "save-by-date", targetCents: 20000, currentCents: 10000, monthlyCents: 10000 };
    // remaining 10000, monthly 10000 -> months = 1
    const { windowMonths } = buildHorizonRows([near]);
    expect(windowMonths).toBe(6);
  });

  it("grows the window dynamically to fit the furthest-out goal", () => {
    const far = { ...baseGoal, id: "f1", goalType: "save-by-date", targetCents: 400000, currentCents: 0, monthlyCents: 20000 };
    // remaining 400000, monthly 20000 -> months = 20
    const { windowMonths } = buildHorizonRows([far]);
    expect(windowMonths).toBe(21);
  });

  it("sorts rows ascending by months (soonest first)", () => {
    const soon = { ...baseGoal, id: "s1", goalType: "save-by-date", targetCents: 100000, currentCents: 90000, monthlyCents: 10000 }; // 1 month
    const later = { ...baseGoal, id: "l1", goalType: "save-by-date", targetCents: 500000, currentCents: 0, monthlyCents: 50000 }; // 10 months
    const { rows } = buildHorizonRows([later, soon]);
    expect(rows.map((r) => r.goal.id)).toEqual(["s1", "l1"]);
  });

  it("flags a goal as needing attention when its projected ETA lands later than its target date", () => {
    // remaining 400000, monthly 20000 -> 20 months out, but committed to a target date only 10 months away
    const behind = { ...baseGoal, id: "b1", goalType: "save-by-date", targetCents: 400000, currentCents: 0, monthlyCents: 20000, targetDate: future(10) };
    const { rows } = buildHorizonRows([behind]);
    expect(rows[0]!.needsAttention).toBe(true);
  });

  it("does not flag a goal as needing attention when it will finish on or before its target date", () => {
    // same 20-month projection, but target date is comfortably further out (25 months)
    const onTrack = { ...baseGoal, id: "ot1", goalType: "save-by-date", targetCents: 400000, currentCents: 0, monthlyCents: 20000, targetDate: future(25) };
    const { rows } = buildHorizonRows([onTrack]);
    expect(rows[0]!.needsAttention).toBe(false);
  });

  it("does not flag a goal with no target date, regardless of its projected ETA", () => {
    const noTargetDate = { ...baseGoal, id: "nt1", goalType: "save-by-date", targetCents: 400000, currentCents: 0, monthlyCents: 20000, targetDate: null };
    const { rows } = buildHorizonRows([noTargetDate]);
    expect(rows[0]!.needsAttention).toBe(false);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ui && npx vitest run src/screens/Goals.test.tsx -t "buildHorizonRows"`
Expected: FAIL — `buildHorizonRows` is not exported from `./Goals` yet.

- [ ] **Step 3: Add `buildHorizonRows` to Goals.tsx**

In `ui/src/screens/Goals.tsx`, insert this new type and function immediately after the existing `etaLabel` function (which currently ends at line 94, right before `function WhatIfScenario({ goals }: { goals: GoalDto[] }) {` on line 96):

```ts
type HorizonRow = {
  goal: GoalDto;
  months: number;
  pct: number;
  xPercent: number;
  needsAttention: boolean;
};

// A goal counts as "behind schedule" if its computed ETA lands later than its
// own targetDate. This is deliberately NOT paceLabel()'s "Needs attention"
// (monthlyCents <= 0): every goal in that state has an infinite ETA and is
// already excluded by the withEta filter below, which would make that branch
// permanently unreachable here. Goals with no targetDate are never flagged.
function isBehindSchedule(goal: GoalDto, months: number): boolean {
  if (!goal.targetDate) return false;
  const eta = new Date();
  eta.setMonth(eta.getMonth() + months);
  return eta.getTime() > new Date(goal.targetDate).getTime();
}

export function buildHorizonRows(goals: GoalDto[]): { rows: HorizonRow[]; windowMonths: number } {
  const eligible = goals.filter((goal) => goal.goalType !== "spending-cap");
  const withEta = eligible
    .map((goal) => ({ goal, months: monthsToGoal(goal) }))
    .filter((entry) => Number.isFinite(entry.months));

  if (withEta.length === 0) return { rows: [], windowMonths: 0 };

  const furthest = Math.max(...withEta.map((entry) => entry.months));
  const windowMonths = Math.max(6, furthest + 1);

  const rows: HorizonRow[] = withEta
    .map(({ goal, months }) => ({
      goal,
      months,
      pct: goal.targetCents > 0 ? Math.min(100, (goal.currentCents / goal.targetCents) * 100) : 0,
      xPercent: (months / windowMonths) * 100,
      needsAttention: isBehindSchedule(goal, months),
    }))
    .sort((a, b) => a.months - b.months);

  return { rows, windowMonths };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ui && npx vitest run src/screens/Goals.test.tsx -t "buildHorizonRows"`
Expected: PASS — all 10 new tests green.

- [ ] **Step 5: Run the full Goals test file and type-check**

Run: `cd ui && npx vitest run src/screens/Goals.test.tsx`
Expected: all tests in the file pass (pre-existing tests unaffected, since `buildHorizonRows` is a brand-new export with no other call site yet).

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/screens/Goals.tsx ui/src/screens/Goals.test.tsx
git commit -m "feat: add buildHorizonRows helper for the Goals timeline"
```

---

### Task 2: Add the `GoalsHorizon` component and wire it into the screen

**Files:**
- Modify: `ui/src/screens/Goals.tsx`
- Modify: `ui/src/screens/Goals.test.tsx`

- [ ] **Step 1: Write the failing tests**

Add this new `describe` block to the end of `ui/src/screens/Goals.test.tsx`:

```ts
describe("Goals — Horizon timeline", () => {
  it("renders a row per eligible goal with name, eta, and target amount", () => {
    render(<Goals />, { wrapper: createWrapper() });
    // g1: Italy Fund, target 500000c, current 100000c, monthly 20000c -> 20 months
    // g2: Car repair, target 200000c, current 50000c, monthly 10000c -> 15 months
    expect(screen.getByText("Horizon")).toBeInTheDocument();
    expect(screen.getByText("When each goal lands.")).toBeInTheDocument();
    expect(screen.getAllByText(/Italy Fund/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Car repair/).length).toBeGreaterThan(0);
  });

  it("hides the Horizon section entirely when no goal has a finite ETA", async () => {
    const budget = await import("../api/hooks/budget");
    (budget.useGoals as ReturnType<typeof vi.fn>).mockReturnValueOnce({
      data: [
        {
          id: "sc1", name: "Dining cap", goalType: "spending-cap",
          targetCents: 40000, currentCents: 10000, monthlyCents: 0,
          targetDate: null, color: "#C9F950", notes: null, purpose: null,
          sortOrder: 0, createdAt: "2026-01-01", liabilityId: null, accountId: null,
        },
      ],
      isLoading: false,
      error: null,
    });
    render(<Goals />, { wrapper: createWrapper() });
    expect(screen.queryByText("Horizon")).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ui && npx vitest run src/screens/Goals.test.tsx -t "Horizon timeline"`
Expected: FAIL — the "Horizon" eyebrow text doesn't exist anywhere on the screen yet.

- [ ] **Step 3: Add the `GoalsHorizon` component**

In `ui/src/screens/Goals.tsx`, insert this new component immediately after the `buildHorizonRows` function added in Task 1 (and before `function WhatIfScenario(...)`):

```tsx
function GoalsHorizon({ goals }: { goals: GoalDto[] }) {
  const { rows, windowMonths } = useMemo(() => buildHorizonRows(goals), [goals]);
  if (rows.length === 0) return null;

  const tickCount = 5;
  const ticks = Array.from({ length: tickCount }, (_, i) => {
    const monthsOut = Math.round((i / (tickCount - 1)) * windowMonths);
    const date = new Date();
    date.setMonth(date.getMonth() + monthsOut);
    return {
      xPercent: (monthsOut / windowMonths) * 100,
      label: date.toLocaleDateString("en-US", { month: "short", year: monthsOut >= 12 ? "2-digit" : undefined }),
    };
  });

  return (
    <section className="section">
      <div className="day-hdr" style={{ marginBottom: 14 }}>
        <div>
          <div className="eyebrow"><span className="dot" />Horizon</div>
          <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>When each goal lands.</h2>
        </div>
      </div>
      <div className="card" style={{ padding: 26 }}>
        <div style={{ position: "relative", height: 20, marginBottom: 8 }}>
          {ticks.map((tick, i) => (
            <span key={i} className="muted mono" style={{ position: "absolute", left: `${tick.xPercent}%`, fontSize: 11 }}>{tick.label}</span>
          ))}
        </div>
        <div style={{ position: "relative", paddingTop: 8 }}>
          <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: 2, background: "var(--accent)", boxShadow: "0 0 8px var(--accent)" }} />
          {rows.map((row) => {
            const color = row.needsAttention ? "var(--negative)" : "var(--accent)";
            return (
              <div key={row.goal.id} style={{ position: "relative", height: 44, display: "flex", alignItems: "center" }}>
                <div style={{ position: "absolute", left: 0, top: "50%", width: `${row.xPercent}%`, height: 1, background: "var(--hairline)" }} />
                <div style={{ position: "absolute", left: 0, top: "50%", width: `${(row.xPercent * row.pct) / 100}%`, height: 2, background: color }} />
                <div style={{ position: "absolute", left: `${row.xPercent}%`, top: "50%", transform: "translate(-50%, -50%)", width: 10, height: 10, borderRadius: "50%", border: `2px solid ${color}`, background: "var(--surface)" }} />
                <div style={{ position: "absolute", left: `calc(${row.xPercent}% + 14px)`, top: "50%", transform: "translateY(-50%)", fontSize: 13, whiteSpace: "nowrap" }}>
                  {row.goal.name} <span className="muted mono" style={{ fontSize: 12 }}>· {etaLabel(row.months)} · {money(row.goal.targetCents, { currency: "USD" })}</span>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 4: Wire `GoalsHorizon` into the main `Goals` component**

In `ui/src/screens/Goals.tsx`, find this block (the goal-cards list, followed by the `WhatIfScenario` render — currently at the end of the file):

```tsx
      <div className="section" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {visible.map((goal) => (
          <GoalCard
            key={goal.id}
            goal={goal}
            onEdit={setEditingGoal}
            liabilityName={goal.liabilityId ? liabilityNameById.get(goal.liabilityId) ?? null : null}
            onTogglePause={(g) => void handleTogglePause(g)}
            pausePending={updateGoalMonthly.isPending}
            pausedByUser={goal.id in pausedPrevious}
          />
        ))}
      </div>

      {goals.length > 0 && <WhatIfScenario goals={goals} />}
```

Replace it with (adds one line — `<GoalsHorizon goals={goals} />` — between the two existing blocks; note this passes the full `goals` array, not the filtered `visible` array, since the Horizon timeline shows all eligible goals regardless of the active type filter, matching the mockup):

```tsx
      <div className="section" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {visible.map((goal) => (
          <GoalCard
            key={goal.id}
            goal={goal}
            onEdit={setEditingGoal}
            liabilityName={goal.liabilityId ? liabilityNameById.get(goal.liabilityId) ?? null : null}
            onTogglePause={(g) => void handleTogglePause(g)}
            pausePending={updateGoalMonthly.isPending}
            pausedByUser={goal.id in pausedPrevious}
          />
        ))}
      </div>

      <GoalsHorizon goals={goals} />

      {goals.length > 0 && <WhatIfScenario goals={goals} />}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ui && npx vitest run src/screens/Goals.test.tsx`
Expected: PASS — all tests in the file, including the two new Horizon tests.

- [ ] **Step 6: Run the full frontend suite and type-check**

Run: `cd ui && npx vitest run`
Expected: all tests pass (212 pre-existing + 10 new `buildHorizonRows` tests from Task 1 + 2 new Horizon-rendering tests here = 224 total).

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add ui/src/screens/Goals.tsx ui/src/screens/Goals.test.tsx
git commit -m "feat: add Horizon timeline section to Goals screen"
```

---

## Final verification (done by the orchestrating session, not a subagent task)

After both tasks are committed:
1. Run `cd ui && npx vitest run` and `cd ui && npx tsc --noEmit` one more time from a clean state to confirm no cross-task regressions.
2. Start the app and visually confirm on the Goals screen: a new "Horizon" section appears between the goal cards and the "What if · scenario" section, showing one row per eligible goal (excluding spending-cap goals and any goal with no monthly contribution), each with a correctly positioned dot, a progress-filled track, and a label showing the goal name, ETA month/year, and target amount. Real seeded goals may or may not include one that's actually behind its own target date — if none are, that's expected (the red "needs attention" branch is still covered by the Task 1 unit tests either way); if one is, confirm it renders in `var(--negative)` (red) instead of `var(--accent)` (green) for its track/dot.
3. Update the Tier B section of `docs/superpowers/plans/2026-07-01-design-conformance-deep-audit.md` to mark the "Goals: Horizon timeline visualization" item as done, with a one-line pointer to this plan and spec.
