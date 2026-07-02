# Goals Horizon Timeline — Design

**Status:** Approved, ready for implementation plan
**Tier:** B (buildable for real, no fabrication) — item 2 of 6, from `docs/superpowers/plans/2026-07-01-design-conformance-deep-audit.md`

## Problem

The Goals screen has no visual answer to "when does each goal land?" The design mockup has a "Horizon" section — a single shared timeline showing every goal's projected finish date as a marker, with a progress-filled track leading up to it. The app has none of this. All the data needed (`monthsToGoal`/`etaLabel`, already defined in `Goals.tsx` and used by the existing "What if · scenario" section) is present client-side — no backend or schema change is needed.

## Research findings

- Design mockup (`components/goals.jsx`, project `fdbc4798-c6d0-41df-9499-e6ca4294d142`), component `GoalHorizon`: a combined view of all goals (not per-goal), rendered as one card:
  - A month-label axis across the top.
  - A vertical glowing accent line at "today" (month 0).
  - One 44px-tall row per goal: a faint hairline track from the left edge to the goal's ETA x-position, an accent-colored progress segment overlaid on that track (scaled by `current/target`), a circular dot marker at the ETA position, and a text label (`"{goal name} · {eta} · {target amount}"`) to the right of the dot.
  - Row/marker color: `var(--negative)` (red) if the goal's pace is "needs attention", otherwise `var(--accent)`.
  - No dedicated `.horizon-*` CSS classes exist anywhere in `styles.css` or `app.css` — the mockup builds this entirely with inline styles reusing existing tokens (`--accent`, `--negative`, `--hairline`, `--mono`).
- `ui/src/screens/Goals.tsx` currently has no Horizon/timeline section. `monthsToGoal(goal, monthlyOverrideCents?)` (returns months until target, or `Infinity` if `monthlyCents <= 0` and not yet complete) and `etaLabel(months)` (formats months-from-now as `"Mon YYYY"`) already exist and are used only by `WhatIfScenario`.
- `GoalDto` (bindings.ts) has everything needed: `targetCents`, `currentCents`, `monthlyCents`, `goalType`, `name` — no new fields required.
- No existing reusable timeline component in the app. `Journey.tsx` has a vertical stage-stepper (different visual language). `.goal-bar` is a simple linear fill, not a positioned-marker-on-axis component. This will be new inline-styled markup, following the mockup's own approach and reusing existing color tokens.

## Decisions (from user Q&A)

1. **Goals with no computable ETA** (`monthsToGoal` returns `Infinity`, i.e. `monthlyCents <= 0` and not yet complete) are **excluded** from the timeline entirely — they stay visible in the goal cards above with their existing "Needs attention" chip, but don't get a row here.
2. **Spending-cap goals are excluded** from the timeline (they reset monthly and have no long-term finish-line concept), matching the existing `WhatIfScenario` filter (`goals.filter((goal) => goal.goalType !== "spending-cap")`).
3. **Placement:** a new section between the goal-cards list and the "What if · scenario" section (the mockup's original position, previously occupied by the now-removed duplicate "Sinking funds" section).
4. **Empty state:** if zero eligible goals have a finite ETA, the entire Horizon section is hidden (no empty timeline card).
5. **Window sizing (implementation decision, not asked as a separate question):** the mockup hardcodes a 14-month window sized for its own mock data. Since real user goals could have any ETA, the window is **dynamic**: `windowMonths = Math.max(6, furthestEligibleMonths + 1)`, so the furthest-out real goal is never clipped off the visible area, with a floor of 6 months so the timeline doesn't look absurdly zoomed-in when all goals are near-term.

## Design

### New pure helper: `buildHorizonRows`

Added to `ui/src/screens/Goals.tsx` (co-located with `monthsToGoal`/`etaLabel`, which it calls):

```ts
type HorizonRow = {
  goal: GoalDto;
  months: number;
  pct: number;        // 0-100, progress fill position (current/target)
  xPercent: number;    // 0-100, marker position along the timeline
  needsAttention: boolean;
};

function buildHorizonRows(goals: GoalDto[]): { rows: HorizonRow[]; windowMonths: number } {
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
      needsAttention: paceLabel(goal).label === "Needs attention",
    }))
    .sort((a, b) => a.months - b.months);

  return { rows, windowMonths };
}
```

This is pure and independently testable — no rendering needed to verify filtering, sorting, and position math.

### New component: `GoalsHorizon`

A new function component in `Goals.tsx`, alongside `WhatIfScenario`:

```tsx
function GoalsHorizon({ goals }: { goals: GoalDto[] }) {
  const { rows, windowMonths } = useMemo(() => buildHorizonRows(goals), [goals]);
  if (rows.length === 0) return null;

  const tickCount = 5;
  const ticks = Array.from({ length: tickCount }, (_, i) => {
    const monthsOut = Math.round((i / (tickCount - 1)) * windowMonths);
    const date = new Date();
    date.setMonth(date.getMonth() + monthsOut);
    return { xPercent: (monthsOut / windowMonths) * 100, label: date.toLocaleDateString("en-US", { month: "short", year: monthsOut >= 12 ? "2-digit" : undefined }) };
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

### Wiring into the screen

In the main `Goals()` component, add `<GoalsHorizon goals={goals} />` between the existing goal-cards `<div className="section" ...>` block and the `{goals.length > 0 && <WhatIfScenario goals={goals} />}` line. `GoalsHorizon` handles its own empty-state (`rows.length === 0` → renders nothing), so no extra conditional is needed at the call site.

### Testing

Add a new test file `ui/src/screens/goalsHorizon.test.ts` (or a `describe` block in the existing `Goals.test.tsx`, whichever the plan picks) covering `buildHorizonRows` directly as a pure function:
- A spending-cap goal is excluded even if it would otherwise have a finite ETA.
- A goal with `monthlyCents: 0` and incomplete progress (infinite ETA) is excluded.
- A goal that's already complete (`currentCents >= targetCents`) gets `months: 0` and is included at `xPercent: 0`.
- `windowMonths` is `Math.max(6, furthest + 1)` — verify both the floor-of-6 case (all goals near-term) and the dynamic-growth case (one goal far out).
- Rows are sorted ascending by `months`.
- `pct` is correctly clamped to 100 when `currentCents > targetCents`.

Add a `Goals.test.tsx` test asserting: the Horizon section doesn't render when all goals are spending-cap or have infinite ETA (no "Horizon" eyebrow text found); it does render with the correct goal names/eta/target text when at least one eligible goal exists.

## Out of scope

- Interactivity (clicking a marker to open the goal drawer) — the mockup doesn't do this either, it's a read-only visual.
- Any change to `monthsToGoal`, `etaLabel`, or `paceLabel` — reused as-is.
- Backend/schema changes — none needed.

## Risks / edge cases

- Many goals with very close ETAs could produce visually overlapping dots/labels at small window sizes — acceptable for now given the mockup itself doesn't handle this case either (out of scope to solve collision detection).
- A goal already past its target date but not yet marked complete would show `months: 0` and sit at the "today" line — this matches `etaLabel(0)`'s existing behavior (today's month/year), not a new edge case introduced by this feature.
