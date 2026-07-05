# Copilot UI Visual Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the Claude Design "Plutus" mockup's Copilot chat visuals (hero, turn header, thinking block, 7 result-card kinds, composer) into the real assistant-ui-backed Copilot screen, backed entirely by real data — no scripted/fake behavior.

**Architecture:** Five ordered phases, each independently shippable and testable: (A) frontend message-shell restyle using data already flowing today, (B) Recharts + a themed wrapper, verified mid-stream before further chart use, (C) six new `AgentResponseBlock` kinds pushed through the existing three-gate validation chain, (D) a new reasoning-engine Plan step threaded through AG-UI only, (E) `RecategorizationPreview` synthesis + a real CSV export command. Full architectural context (why each decision was made, what was explicitly rejected) lives in `docs/superpowers/specs/2026-07-05-copilot-ui-visual-redesign.md` — read it before starting if anything below is unclear.

**Tech Stack:** Rust (finsight-core/finsight-agent/finsight-app crates, rusqlite, specta/tauri-specta bindings), React 18 + TypeScript (assistant-ui `@assistant-ui/react` 0.14.24, Vitest, Zod), Recharts (new dependency).

---

## Phase A: Frontend message-shell restyle

No backend changes. Every task in this phase uses data the app already has (real tool-call args/results, real `MessageMeta`, real grounding stats). This phase is independently shippable and fully testable via Vitest — no native app required.

### Task A1: Add the `.cp-*` shell CSS

**Files:**
- Create: `ui/src/styles/copilot-shell.css`
- Modify: `ui/src/screens/Copilot.tsx:1` (add the import)

The mockup CSS (`copilot.css` in the Claude Design "Plutus" project) already uses FinSight's own CSS variables (`var(--accent)`, `var(--surface)`, `var(--ink)`, etc.), so it ports directly. Using a **new** `.cp-*` namespace (distinct from the existing ~400 `.copilot-*` rules already in `app.css`) avoids collisions — existing `.copilot-*` rules for parts not touched by this redesign (history popover, screen layout, message bubbles for user turns) are untouched.

- [ ] **Step 1: Create the new stylesheet**

```css
/* ui/src/styles/copilot-shell.css
   New visual layer for the redesigned Copilot message shell, ported from
   the Claude Design "Plutus" mockup (project fdbc4798-c6d0-41df-9499-e6ca4294d142,
   copilot.css). Namespaced `.cp-*` to avoid colliding with the existing
   ~400 `.copilot-*` rules in app.css, which still cover parts of the
   screen this redesign doesn't touch (history popover, screen chrome). */

/* ── Hero / empty state ─────────────────────────────────────────────── */
.cp-hero { position: relative; width: 100%; overflow: hidden; min-height: calc(100vh - 200px); display: grid; place-items: center; }
.cp-hero-glow { position: absolute; inset: 0; pointer-events: none; overflow: hidden; }
.cp-glow-orb { position: absolute; border-radius: 999px; filter: blur(80px); opacity: 0.55; }
.cp-glow-1 { width: 640px; height: 480px; background: radial-gradient(ellipse, oklch(80% 0.22 135 / 0.18) 0%, transparent 70%); top: -120px; left: 50%; transform: translateX(-50%); animation: cp-drift 12s ease-in-out infinite; }
.cp-glow-2 { width: 360px; height: 300px; background: radial-gradient(ellipse, oklch(60% 0.15 280 / 0.10) 0%, transparent 70%); bottom: 80px; right: 5%; animation: cp-drift2 16s ease-in-out infinite reverse; }
@keyframes cp-drift { 0%, 100% { transform: translateX(-50%) translateY(0); } 50% { transform: translateX(-50%) translateY(-24px); } }
@keyframes cp-drift2 { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(16px); } }
.cp-hero-inner { position: relative; z-index: 1; max-width: 680px; width: 100%; display: flex; flex-direction: column; align-items: center; text-align: center; }
.cp-hero-avatar { width: 52px; height: 52px; border-radius: 16px; background: var(--accent-2); border: 1px solid var(--accent-3); display: grid; place-items: center; margin-bottom: 28px; box-shadow: 0 0 40px var(--accent-2), inset 0 1px 0 rgba(255,255,255,0.08); }
.cp-avatar-ring { width: 26px; height: 26px; border-radius: 9px; background: var(--accent); display: grid; place-items: center; box-shadow: 0 0 16px var(--accent); }
.cp-avatar-core { width: 10px; height: 10px; border-radius: 4px; background: var(--accent-ink); }
.cp-hero-h1 { font-family: var(--sans); font-size: 48px; font-weight: 600; letter-spacing: -0.04em; line-height: 1.04; color: var(--ink); margin: 0 0 14px; text-wrap: balance; }
.cp-hero-sub { font-size: 17px; line-height: 1.5; color: var(--ink-mute); margin: 0 0 36px; max-width: 48ch; text-wrap: pretty; }
.cp-hero-chips { display: flex; flex-wrap: wrap; justify-content: center; gap: 8px; margin-top: 28px; }
.cp-hero-chip { display: inline-flex; align-items: center; gap: 8px; padding: 9px 16px; border-radius: 999px; border: 1px solid var(--line-2); background: var(--surface); color: var(--ink-2); font-size: 13.5px; cursor: pointer; transition: color .13s, border-color .13s, background .13s, transform .13s, box-shadow .13s; }
.cp-hero-chip:hover { color: var(--ink); border-color: var(--accent-3); background: color-mix(in oklab, var(--accent-2) 60%, var(--surface)); transform: translateY(-1px); box-shadow: 0 4px 16px rgba(0,0,0,0.3); }
.cp-chip-ico { color: var(--accent); flex-shrink: 0; }
.cp-hero-ground { display: flex; flex-wrap: wrap; justify-content: center; gap: 20px; margin-top: 28px; }
.cp-hero-ground span { display: inline-flex; align-items: center; gap: 6px; font-family: var(--mono); font-size: 11px; color: var(--ink-faint); }
.cp-hero-ground svg { opacity: 0.6; }

/* ── Turn header ────────────────────────────────────────────────────── */
.cp-turn-hd { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-bottom: 10px; }
.cp-agent-mark { width: 26px; height: 26px; border-radius: 8px; flex-shrink: 0; background: var(--accent-2); border: 1px solid var(--accent-3); display: grid; place-items: center; }
.cp-agent-core { width: 10px; height: 10px; border-radius: 3.5px; background: var(--accent); box-shadow: 0 0 8px var(--accent); }
.cp-agent-mark.is-thinking .cp-agent-core { animation: cp-think-pulse 1.2s ease-in-out infinite; }
@keyframes cp-think-pulse { 0%, 100% { opacity: 1; transform: scale(1); } 50% { opacity: 0.4; transform: scale(0.65); border-radius: 999px; } }
.cp-turn-name { font-size: 14px; font-weight: 600; color: var(--ink); letter-spacing: -0.01em; }
.cp-turn-model { font-family: var(--mono); font-size: 11px; color: var(--ink-faint); }
.cp-src-rail { display: flex; gap: 5px; flex-wrap: wrap; margin-left: auto; }
.cp-src { display: inline-flex; align-items: center; gap: 5px; font-family: var(--mono); font-size: 10px; color: var(--ink-faint); padding: 2px 8px; border-radius: 999px; border: 1px solid var(--hairline); transition: color .25s, border-color .25s, background .25s; }
.cp-src.is-on { color: var(--accent); border-color: var(--accent-3); background: var(--accent-2); }

/* ── Thinking block ─────────────────────────────────────────────────── */
.cp-think { border: 1px solid var(--line); border-radius: 12px; background: var(--bg-2); overflow: hidden; margin-bottom: 12px; }
.cp-think.is-running { border-color: var(--accent-3); }
.cp-think-hd { width: 100%; display: flex; align-items: center; gap: 10px; padding: 12px 16px; text-align: left; cursor: pointer; background: none; border: none; }
.cp-think-hd:hover { background: var(--surface); }
.cp-think-ico { width: 22px; height: 22px; border-radius: 6px; background: var(--surface-2); border: 1px solid var(--line); display: grid; place-items: center; flex-shrink: 0; color: var(--accent); }
.cp-think.is-done .cp-think-ico { background: var(--accent-2); border-color: var(--accent-3); }
.cp-think-dots { display: inline-flex; align-items: center; gap: 3px; }
.cp-think-dots i { display: block; width: 4px; height: 4px; border-radius: 999px; background: var(--accent); animation: cp-dot 1.4s ease-in-out infinite; }
.cp-think-dots i:nth-child(2) { animation-delay: .16s; }
.cp-think-dots i:nth-child(3) { animation-delay: .32s; }
@keyframes cp-dot { 0%, 80%, 100% { opacity: 0.25; transform: scale(0.75); } 40% { opacity: 1; transform: scale(1); } }
.cp-think-title { font-size: 13px; font-weight: 500; color: var(--ink); }
.cp-think-chev { width: 14px; height: 14px; color: var(--ink-faint); margin-left: auto; transition: transform .18s; }
.cp-think-chev.open { transform: rotate(180deg); }
.cp-think-body { border-top: 1px solid var(--hairline); padding: 4px 18px 18px; display: flex; flex-direction: column; }
.cp-think-sec { padding-top: 16px; }
.cp-think-sec-lbl { font-family: var(--mono); font-size: 10px; letter-spacing: 0.09em; text-transform: uppercase; color: var(--ink-faint); margin: 0 0 10px; }
.cp-think-plan { display: flex; flex-direction: column; gap: 7px; }
.cp-plan-item { display: flex; align-items: flex-start; gap: 10px; }
.cp-plan-n { width: 18px; height: 18px; border-radius: 5px; flex-shrink: 0; background: var(--surface-2); border: 1px solid var(--line); color: var(--ink-mute); font-family: var(--mono); font-size: 10px; font-weight: 600; display: grid; place-items: center; }
.cp-plan-txt { font-size: 13px; color: var(--ink-2); line-height: 1.5; padding-top: 1px; }
.cp-think-tools { display: flex; flex-direction: column; gap: 5px; }
.cp-tool { border: 1px solid var(--line); border-radius: 8px; background: var(--surface); overflow: hidden; }
.cp-tool-row { width: 100%; display: flex; align-items: center; gap: 10px; padding: 8px 12px; text-align: left; background: none; border: none; cursor: pointer; }
.cp-tool-row:hover { background: var(--surface-2); }
.cp-tool-dot { width: 18px; height: 18px; border-radius: 999px; flex-shrink: 0; display: grid; place-items: center; background: var(--accent-2); color: var(--accent); }
.cp-tool-sig { font-size: 12px; min-width: 0; flex-shrink: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cp-tool-fn { color: var(--ink); font-weight: 600; }
.cp-tool-args { color: var(--ink-faint); }
.cp-tool-result { margin-left: auto; flex-shrink: 0; font-family: var(--mono); font-size: 11px; color: var(--accent); background: var(--accent-2); padding: 2px 9px; border-radius: 999px; }
.cp-tool-result.is-error { color: var(--negative); background: var(--negative-2); }
.cp-tool-chev { width: 13px; height: 13px; color: var(--ink-faint); flex-shrink: 0; transition: transform .17s; }
.cp-tool-chev.open { transform: rotate(180deg); }
.cp-tool-detail { padding: 0 12px 12px 40px; }
.cp-tool-pre { font-family: var(--mono); font-size: 11.5px; line-height: 1.7; color: var(--ink-2); background: var(--bg-2); border: 1px solid var(--hairline); border-radius: 7px; padding: 10px 13px; margin: 0; white-space: pre-wrap; }
.cp-think-reason { display: flex; flex-direction: column; gap: 12px; }
.cp-reason-item { display: grid; grid-template-columns: 22px 1fr; gap: 12px; align-items: start; position: relative; }
.cp-reason-item:not(:last-child)::before { content: ""; position: absolute; left: 10px; top: 22px; bottom: -12px; width: 1px; background: linear-gradient(to bottom, var(--accent-3), transparent); }
.cp-reason-n { width: 21px; height: 21px; border-radius: 999px; background: var(--surface); border: 1.5px solid var(--accent); color: var(--accent); font-family: var(--mono); font-size: 10px; font-weight: 600; display: grid; place-items: center; flex-shrink: 0; z-index: 1; }
.cp-reason-txt { font-size: 13px; line-height: 1.6; color: var(--ink-2); text-wrap: pretty; padding-top: 1px; margin: 0; }

/* ── Composer ───────────────────────────────────────────────────────── */
.cp-composer-model { display: inline-flex; align-items: center; gap: 8px; font-family: var(--mono); font-size: 11px; color: var(--ink-faint); }
.cp-model-dot { width: 6px; height: 6px; border-radius: 999px; background: var(--accent); box-shadow: 0 0 6px var(--accent); animation: cp-pulse-dot 2.4s ease-in-out infinite; }
@keyframes cp-pulse-dot { 50% { opacity: 0.4; box-shadow: none; } }

/* ── Turn footer + follow-ups (restyle of existing real elements) ────── */
.cp-turn-ft { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; font-family: var(--mono); font-size: 10.5px; color: var(--ink-faint); margin-top: 10px; }
.cp-turn-ft svg { opacity: 0.6; }
.cp-followups { display: flex; flex-direction: column; gap: 10px; padding-top: 6px; margin-top: 14px; }
.cp-followups-lbl { font-family: var(--mono); font-size: 10px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--ink-faint); }
.cp-followups-row { display: flex; flex-wrap: wrap; gap: 7px; }
.cp-fu-chip { font-size: 13px; color: var(--ink-2); background: var(--surface); border: 1px solid var(--line); border-radius: 999px; padding: 6px 14px; cursor: pointer; white-space: nowrap; transition: color .12s, border-color .12s, background .12s, transform .12s; }
.cp-fu-chip:hover { color: var(--ink); border-color: var(--accent-3); background: var(--accent-2); transform: translateY(-1px); }

@media (prefers-reduced-motion: reduce) {
  .cp-glow-orb, .cp-agent-mark.is-thinking .cp-agent-core, .cp-think-dots i, .cp-model-dot { animation: none; }
}
```

- [ ] **Step 2: Import it in Copilot.tsx**

Add near the top of `ui/src/screens/Copilot.tsx` (after the existing `import "streamdown/styles.css";` line):

```ts
import "../styles/copilot-shell.css";
```

- [ ] **Step 3: Verify the build picks it up**

Run: `cd ui && npx tsc --noEmit`
Expected: no new errors (CSS imports aren't type-checked, this just confirms nothing else broke).

- [ ] **Step 4: Commit**

```bash
git add ui/src/styles/copilot-shell.css ui/src/screens/Copilot.tsx
git commit -m "style(copilot): add the Plutus-mockup shell CSS namespace"
```

### Task A2: Restyle the hero/empty state with real grounding data

**Files:**
- Modify: `ui/src/screens/Copilot.tsx` (`CopilotGroundingStats`, `EmptyThreadState`, `SUGGESTED_PROMPTS`)
- Test: create `ui/src/screens/Copilot.hero.test.tsx`

Current `EmptyThreadState` (Copilot.tsx:555-589) renders a kicker/title/sub, `CopilotGroundingStats`, then a `SUGGESTED_PROMPTS` grid of `.copilot-prompt-card` elements. Replace the prompt grid with pill chips (`.cp-hero-chip`) and wrap everything in the `.cp-hero`/`.cp-hero-glow`/`.cp-hero-inner` structure. Keep `CopilotGroundingStats`'s real counts and its honest "no data yet" branch completely unchanged — only the wrapping markup/classes change.

- [ ] **Step 1: Write the failing test**

```tsx
// ui/src/screens/Copilot.hero.test.tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { createWrapper } from "../test-utils";

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [{ id: "a1" }], isLoading: false, error: null })),
}));
vi.mock("../api/client", async () => {
  const actual = await vi.importActual("../api/client");
  return {
    ...actual,
    commands: {
      ...((actual as any).commands),
      getTransactionCount: vi.fn().mockResolvedValue({ status: "ok", data: 42 }),
    },
  };
});
vi.mock("../components/copilot/TauriRuntime", () => ({
  useTauriCopilotRuntime: () => ({
    runtime: { thread: { getState: () => ({ messages: [] }) }, subscribe: () => () => {} },
    latestMeta: null,
    metaByMessageId: {},
  }),
}));
vi.mock("../components/copilot/agUi/featureFlag", () => ({
  isCopilotAgUiRuntimeEnabled: () => false,
}));

import Copilot from "./Copilot";

describe("Copilot hero", () => {
  it("renders the hero shell with real grounding stats, not hardcoded mockup numbers", async () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(await screen.findByText(/42 transaction/i)).toBeInTheDocument();
    expect(screen.queryByText(/1,247 transaction/i)).not.toBeInTheDocument();
  });

  it("renders suggestion chips instead of the old prompt-card grid", async () => {
    render(<Copilot />, { wrapper: createWrapper() });
    const chip = await screen.findByRole("button", { name: /Plan next month's budget/i });
    expect(chip.className).toContain("cp-hero-chip");
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd ui && npx vitest run src/screens/Copilot.hero.test.tsx`
Expected: FAIL — no element with class `cp-hero-chip` exists yet, and the current markup is `.copilot-prompt-card`.

- [ ] **Step 3: Restyle `EmptyThreadState` and `SUGGESTED_PROMPTS` rendering**

Replace the body of `EmptyThreadState` (Copilot.tsx:555-589) with:

```tsx
function EmptyThreadState({
  onPrompt,
  children,
}: {
  onPrompt: (text: string) => void;
  children: ReactNode;
}) {
  const h = new Date().getHours();
  const greeting = h < 12 ? "Good morning" : h < 17 ? "Good afternoon" : "Good evening";

  return (
    <div className="cp-hero">
      <div className="cp-hero-glow" aria-hidden="true">
        <div className="cp-glow-orb cp-glow-1" />
        <div className="cp-glow-orb cp-glow-2" />
      </div>
      <div className="cp-hero-inner">
        <div className="cp-hero-avatar">
          <span className="cp-avatar-ring">
            <span className="cp-avatar-core" />
          </span>
        </div>
        <h1 className="cp-hero-h1">{greeting}.</h1>
        <p className="cp-hero-sub">
          Ask for a plan, explanation, cleanup pass, or tradeoff analysis. FinSight can use
          your local accounts, budgets, goals, and transactions when a tool is needed.
        </p>
        {children}
        <div className="cp-hero-chips">
          {SUGGESTED_PROMPTS.map((p) => (
            <button
              key={p.label}
              type="button"
              className="cp-hero-chip"
              onClick={() => onPrompt(p.label)}
              title={p.detail}
            >
              <I.Sparkle width={12} height={12} className="cp-chip-ico" />
              <span>{p.label}</span>
            </button>
          ))}
        </div>
        <CopilotGroundingStats />
      </div>
    </div>
  );
}
```

`CopilotGroundingStats` itself does not need to change — it already renders real counts and the honest empty-data message; it's just relocated to sit inside `.cp-hero-inner` below the chips instead of above the (now-removed) prompt-card grid.

- [ ] **Step 4: Run the test again**

Run: `cd ui && npx vitest run src/screens/Copilot.hero.test.tsx`
Expected: PASS.

- [ ] **Step 5: Run the full frontend suite to check for regressions**

Run: `cd ui && npx vitest run`
Expected: any pre-existing test asserting `.copilot-prompt-card` or the old prompt-grid text must be updated to match the new chip markup — check `Copilot.test.tsx` if it exists (`Glob ui/src/screens/Copilot.test.tsx`) and adjust only the presentational assertions that changed.

- [ ] **Step 6: Commit**

```bash
git add ui/src/screens/Copilot.tsx ui/src/screens/Copilot.hero.test.tsx
git commit -m "style(copilot): restyle hero/empty state with pill chips, real grounding stats kept"
```

### Task A3: Turn header — agent mark, name, model, source rail

**Files:**
- Modify: `ui/src/screens/Copilot.tsx` (`AssistantMessage`)
- Create: `ui/src/components/copilot/toolSources.ts`
- Test: `ui/src/components/copilot/toolSources.test.ts`

The source rail is derived client-side from which tools were called this turn (`meta.toolTrace`, already populated — see `MessageMeta.toolTrace` in `TauriRuntime.ts:35`). Each trace entry looks like `"Called tool: search_transactions"` (see `engine/mod.rs:66`). Build a small lookup mapping tool name → a source label, matching the mockup's `SOURCES` map.

- [ ] **Step 1: Write the failing test**

```ts
// ui/src/components/copilot/toolSources.test.ts
import { describe, it, expect } from "vitest";
import { sourcesFromToolTrace } from "./toolSources";

describe("sourcesFromToolTrace", () => {
  it("maps known tool names to source labels, de-duplicated and in first-seen order", () => {
    const trace = [
      "Called tool: search_transactions",
      "Called tool: get_goals",
      "Called tool: search_transactions",
    ];
    expect(sourcesFromToolTrace(trace)).toEqual(["Transactions", "Goals"]);
  });

  it("ignores trace lines that aren't tool calls and unknown tool names", () => {
    const trace = ["Tool error: some_unknown_tool", "Called tool: get_budgets"];
    expect(sourcesFromToolTrace(trace)).toEqual(["Budget"]);
  });

  it("returns an empty array for an empty or undefined trace", () => {
    expect(sourcesFromToolTrace(undefined)).toEqual([]);
    expect(sourcesFromToolTrace([])).toEqual([]);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd ui && npx vitest run src/components/copilot/toolSources.test.ts`
Expected: FAIL — `./toolSources` module does not exist.

- [ ] **Step 3: Implement**

```ts
// ui/src/components/copilot/toolSources.ts

/** Maps a tool name prefix/substring to the mockup's source-rail label. */
const TOOL_TO_SOURCE: Array<[match: RegExp, label: string]> = [
  [/transaction/i, "Transactions"],
  [/account|liquid|balance/i, "Accounts"],
  [/liabilit|debt/i, "Liabilities"],
  [/goal/i, "Goals"],
  [/budget/i, "Budget"],
  [/recurring/i, "Subscriptions"],
  [/categor/i, "Categories"],
];

/**
 * Derives the ordered, de-duplicated list of data-source labels touched
 * this turn from `MessageMeta.toolTrace` entries shaped like
 * "Called tool: search_transactions" (see engine/mod.rs's `trace.push`).
 */
export function sourcesFromToolTrace(trace: string[] | undefined): string[] {
  if (!trace || trace.length === 0) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const line of trace) {
    const m = /^Called tool: (\S+)/.exec(line);
    if (!m) continue;
    const toolName = m[1]!;
    const hit = TOOL_TO_SOURCE.find(([re]) => re.test(toolName));
    if (!hit) continue;
    if (seen.has(hit[1])) continue;
    seen.add(hit[1]);
    out.push(hit[1]);
  }
  return out;
}
```

- [ ] **Step 4: Run the test again**

Run: `cd ui && npx vitest run src/components/copilot/toolSources.test.ts`
Expected: PASS.

- [ ] **Step 5: Wire the turn header into `AssistantMessage`**

In `ui/src/screens/Copilot.tsx`, import the new helper near the top:

```ts
import { sourcesFromToolTrace } from "../components/copilot/toolSources";
```

Then, inside `AssistantMessage` (Copilot.tsx:368-509), add the header markup right before `<div className="copilot-bubble-asst">` (currently line 404):

```tsx
        <div className="cp-turn-hd">
          <div className={`cp-agent-mark ${isRunning ? "is-thinking" : ""}`}>
            <span className="cp-agent-core" />
          </div>
          <span className="cp-turn-name">Copilot</span>
          {meta?.modelId && <span className="cp-turn-model">{meta.providerId} · {meta.modelId}</span>}
          <div className="cp-src-rail">
            {sourcesFromToolTrace(meta?.toolTrace).map((label) => (
              <span key={label} className="cp-src is-on">{label}</span>
            ))}
          </div>
        </div>
```

- [ ] **Step 6: Run the full frontend suite**

Run: `cd ui && npx vitest run`
Expected: all pass; if any existing `AssistantMessage`-rendering test snapshots exact DOM structure, update only the added header markup's presence, not unrelated assertions.

- [ ] **Step 7: Commit**

```bash
git add ui/src/components/copilot/toolSources.ts ui/src/components/copilot/toolSources.test.ts ui/src/screens/Copilot.tsx
git commit -m "feat(copilot): add turn header with agent mark, model, and source rail"
```

### Task A4: Restyle the thinking block (Tool calls + Reasoning; Plan added later in Phase D)

**Files:**
- Modify: `ui/src/screens/Copilot.tsx` (`ReasoningGroup`, `ToolFallbackCard`, `AssistantMessage`'s `GroupedParts` render function)
- Test: `ui/src/screens/Copilot.thinking.test.tsx`

Today, `ReasoningGroup` (Copilot.tsx:323-333) is a plain `<details>`/`<summary>` wrapping reasoning text and tool-call parts together. Split it into the mockup's two-part (Plan added in Phase D) collapsible: a header showing running/done state, and a body with a "Tool calls" section (restyled `ToolFallbackCard` rows, expandable) and a "Reasoning" section (existing reasoning text, now rendered as numbered steps by splitting on sentence boundaries — reasoning is a single string today, and splitting it into displayable "steps" for the connector-line visual is a presentation-only transform, not a data change).

- [ ] **Step 1: Write the failing test**

```tsx
// ui/src/screens/Copilot.thinking.test.tsx
import { describe, it, expect } from "vitest";
import { splitReasoningIntoSteps } from "./Copilot";

describe("splitReasoningIntoSteps", () => {
  it("splits on sentence-ending punctuation followed by a space and a capital letter", () => {
    const text = "Housing is fixed. Dining is the lever. It's 13% over average.";
    expect(splitReasoningIntoSteps(text)).toEqual([
      "Housing is fixed.",
      "Dining is the lever.",
      "It's 13% over average.",
    ]);
  });

  it("returns a single-element array for text with no sentence breaks", () => {
    expect(splitReasoningIntoSteps("Just one clause")).toEqual(["Just one clause"]);
  });

  it("returns an empty array for empty/whitespace-only input", () => {
    expect(splitReasoningIntoSteps("")).toEqual([]);
    expect(splitReasoningIntoSteps("   ")).toEqual([]);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd ui && npx vitest run src/screens/Copilot.thinking.test.tsx`
Expected: FAIL — `splitReasoningIntoSteps` is not exported from `./Copilot`.

- [ ] **Step 3: Implement the splitter and export it**

Add near the top of `ui/src/screens/Copilot.tsx`, after the existing helper functions (e.g. after `actionKindLabel`, around line 111):

```ts
/**
 * Presentation-only transform: the backend's `reasoning` field is one joined
 * string (see ReasoningResult.reasoning in engine/mod.rs), not a structured
 * list. Splitting it into sentence-shaped steps lets the thinking block show
 * a numbered, connected list like the mockup without any backend change.
 */
export function splitReasoningIntoSteps(text: string): string[] {
  const trimmed = text.trim();
  if (!trimmed) return [];
  return trimmed
    .split(/(?<=[.!?])\s+(?=[A-Z])/)
    .map((s) => s.trim())
    .filter(Boolean);
}
```

- [ ] **Step 4: Run the test again**

Run: `cd ui && npx vitest run src/screens/Copilot.thinking.test.tsx`
Expected: PASS.

- [ ] **Step 5: Restyle `ReasoningGroup` and the tool-call rendering**

Replace `ReasoningGroup` (Copilot.tsx:323-333) with a stateful collapsible taking the reasoning text and tool-call children separately:

```tsx
function ThinkingBlock({ reasoningText, toolCalls }: { reasoningText: string; toolCalls: ReactNode }) {
  const message = useMessage();
  const isRunning = message.status?.type === "running";
  const [open, setOpen] = useState(isRunning);
  const steps = splitReasoningIntoSteps(reasoningText);

  return (
    <div className={`cp-think ${isRunning ? "is-running" : "is-done"}`}>
      <button type="button" className="cp-think-hd" onClick={() => setOpen((o) => !o)}>
        <span className="cp-think-ico">
          {isRunning ? (
            <span className="cp-think-dots"><i /><i /><i /></span>
          ) : (
            <I.Check width={12} height={12} />
          )}
        </span>
        <span className="cp-think-title">
          {isRunning ? "Reasoning through your data…" : "Reasoned through your data"}
        </span>
        <I.Down className={`cp-think-chev ${open ? "open" : ""}`} width={14} height={14} />
      </button>
      {open && (
        <div className="cp-think-body">
          <div className="cp-think-sec">
            <p className="cp-think-sec-lbl">Tool calls</p>
            <div className="cp-think-tools">{toolCalls}</div>
          </div>
          {steps.length > 0 && (
            <div className="cp-think-sec">
              <p className="cp-think-sec-lbl">Reasoning</p>
              <div className="cp-think-reason">
                {steps.map((step, i) => (
                  <div key={i} className="cp-reason-item">
                    <span className="cp-reason-n">{i + 1}</span>
                    <p className="cp-reason-txt">{step}</p>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
```

Restyle `ToolFallbackCard` (Copilot.tsx:335-345) into an expandable row matching `.cp-tool-row`:

```tsx
function ToolFallbackCard({ part }: { part: { toolName: string; args?: unknown; result?: unknown; isError?: boolean; status?: { type: string } } }) {
  const [open, setOpen] = useState(false);
  const done = part.status?.type !== "running";
  const argsText = part.args && Object.keys(part.args as object).length > 0 ? JSON.stringify(part.args) : "";
  const resultSummary = (() => {
    const r = part.result as { summary?: string } | undefined;
    if (part.isError) return "error";
    if (r && typeof r.summary === "string") return r.summary;
    return done ? "done" : "running…";
  })();

  return (
    <div className={`cp-tool ${done ? "is-done" : "is-running"}`}>
      <button type="button" className="cp-tool-row" onClick={() => done && setOpen((o) => !o)}>
        <span className={`cp-tool-dot ${part.isError ? "is-error" : ""}`}>
          {done ? <I.Check width={10} height={10} /> : <span className="copilot-cursor" aria-hidden="true" />}
        </span>
        <span className="cp-tool-sig">
          <span className="cp-tool-fn">{part.toolName.replaceAll("_", " ")}</span>
          {argsText && <span className="cp-tool-args"> ({argsText})</span>}
        </span>
        <span className={`cp-tool-result ${part.isError ? "is-error" : ""}`}>{resultSummary}</span>
        {done && <I.Down className={`cp-tool-chev ${open ? "open" : ""}`} width={13} height={13} />}
      </button>
      {done && open && (
        <div className="cp-tool-detail">
          <pre className="cp-tool-pre">{JSON.stringify(part.result ?? part.args ?? {}, null, 2)}</pre>
        </div>
      )}
    </div>
  );
}
```

Finally, update `AssistantMessage`'s `GroupedParts` render (Copilot.tsx:411-455): the `"group-thought"` case currently wraps children in `<ReasoningGroup>`. Since the new `ThinkingBlock` needs the reasoning *text* and the tool-call *nodes* as separate props rather than nested children, change the grouping so reasoning text is captured directly:

```tsx
              {({ part, children }) => {
                switch (part.type) {
                  case "group-thought": {
                    const reasoningText = plainText; // already computed above via message.content filter
                    return <ThinkingBlock reasoningText={reasoningText} toolCalls={children} />;
                  }
```

(`plainText` already exists earlier in `AssistantMessage` — Copilot.tsx:379-382 — as the joined text of all `"text"` parts; reuse it rather than adding a new extraction.)

- [ ] **Step 6: Run the full frontend suite**

Run: `cd ui && npx vitest run`
Expected: all pass. Fix any test that asserted the old `"Analysis path"` `<summary>` text — it's replaced by `"Reasoned through your data"` / `"Reasoning through your data…"`.

- [ ] **Step 7: Commit**

```bash
git add ui/src/screens/Copilot.tsx ui/src/screens/Copilot.thinking.test.tsx
git commit -m "style(copilot): restyle thinking block into tool-call rows + numbered reasoning"
```

### Task A5: Restyle composer and follow-up/footer classes

**Files:**
- Modify: `ui/src/screens/Copilot.tsx` (`CopilotComposerBox`, follow-up chips, message meta footer)

Pure class-name changes — no new state, no new tests needed beyond the existing suite passing (these elements' text content doesn't change, only their `className`).

- [ ] **Step 1: Update `CopilotComposerBox`'s footer row** (Copilot.tsx:715-749): add a model badge matching `.cp-composer-model` inside the composer, near the send button:

```tsx
      <div className="cp-composer-model">
        <span className="cp-model-dot" />
        <span>Local · FinSight Copilot</span>
      </div>
```

Place it as a sibling before `ComposerPrimitive.Input`, inside `ComposerPrimitive.Root`.

- [ ] **Step 2: Restyle the follow-up chips and footer** in `AssistantMessage` (Copilot.tsx:465-504): change `className="chip"` → `className="cp-fu-chip"` on the follow-up buttons, wrap the follow-up section in `className="cp-followups"` with a `<span className="cp-followups-lbl">Ask next</span>` label (replacing "Follow-up suggestions"), and change the meta row's className from `copilot-msg-meta` to `cp-turn-ft` (keep the same real content — grounded badge, model, elapsed time, tool count).

- [ ] **Step 3: Run the full frontend suite**

Run: `cd ui && npx vitest run`
Expected: pass; update any test asserting the literal text `"Follow-up suggestions"` to `"Ask next"`.

- [ ] **Step 4: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/Copilot.tsx
git commit -m "style(copilot): restyle composer model badge and follow-up/footer chips"
```

---

## Phase B: Recharts + themed wrapper, verified mid-stream

### Task B1: Add Recharts and build `FinSightChart`

**Files:**
- Modify: `ui/package.json` (add dependency)
- Create: `ui/src/components/copilot/charts/FinSightChart.tsx`
- Test: `ui/src/components/copilot/charts/FinSightChart.test.tsx`

Per the design decision: Recharts is used only for genuinely chart-shaped visuals, and default Recharts styling never ships — this wrapper is the one place chart theming lives.

- [ ] **Step 1: Add the dependency**

Run: `cd ui && npm install recharts@2`
Expected: `package.json`/`package-lock.json` updated with `recharts` under `dependencies`.

- [ ] **Step 2: Write the failing test**

```tsx
// ui/src/components/copilot/charts/FinSightChart.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { FinSightBarComparison } from "./FinSightChart";

describe("FinSightBarComparison", () => {
  it("renders both labeled bars with formatted currency values", () => {
    render(
      <FinSightBarComparison
        title="Dining · this month vs average"
        current={{ label: "May 2026", amountCents: 41200 }}
        prior={{ label: "12-mo avg", amountCents: 36500 }}
      />
    );
    expect(screen.getByText("Dining · this month vs average")).toBeInTheDocument();
    expect(screen.getByText("May 2026")).toBeInTheDocument();
    expect(screen.getByText("12-mo avg")).toBeInTheDocument();
    expect(screen.getByText("$412")).toBeInTheDocument();
    expect(screen.getByText("$365")).toBeInTheDocument();
  });

  it("shows an empty state instead of a chart when both values are zero", () => {
    render(
      <FinSightBarComparison
        title="No data"
        current={{ label: "This month", amountCents: 0 }}
        prior={{ label: "Last month", amountCents: 0 }}
      />
    );
    expect(screen.getByText(/no comparison data/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cd ui && npx vitest run src/components/copilot/charts/FinSightChart.test.tsx`
Expected: FAIL — module does not exist.

- [ ] **Step 4: Implement the themed wrapper**

```tsx
// ui/src/components/copilot/charts/FinSightChart.tsx
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Cell, ResponsiveContainer } from "recharts";
import { money } from "../../../utils/format";

export type MoneyPoint = { label: string; amountCents: number };

/**
 * Shared theming for every Recharts-based Copilot card: FinSight's own
 * typography/grid/axis colors via CSS variables, currency-formatted values,
 * and an explicit empty state — never raw Recharts defaults.
 *
 * NOTE on measured width: this reads its container's width via
 * ResponsiveContainer, which renders blank at width:0 and can re-animate on
 * every reflow. FinSightBarComparison must only be mounted once the
 * assistant message has finished streaming (`isRunning === false`) — see
 * Task B2's mid-stream verification and the call site in renderers.tsx.
 */
export function FinSightBarComparison({
  title,
  current,
  prior,
}: {
  title?: string;
  current: MoneyPoint;
  prior: MoneyPoint;
}) {
  if (current.amountCents === 0 && prior.amountCents === 0) {
    return (
      <div className="cp-card">
        {title && <p className="cp-card-title">{title}</p>}
        <p className="muted" style={{ fontSize: 12.5 }}>No comparison data available.</p>
      </div>
    );
  }

  const data = [
    { name: prior.label, value: prior.amountCents / 100, isCurrent: false },
    { name: current.label, value: current.amountCents / 100, isCurrent: true },
  ];

  return (
    <div className="cp-card">
      {title && <p className="cp-card-title" style={{ marginBottom: 12 }}>{title}</p>}
      <div style={{ width: "100%", height: 120 }}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data} layout="vertical" margin={{ top: 4, right: 24, bottom: 4, left: 4 }}>
            <CartesianGrid horizontal={false} stroke="var(--line)" />
            <XAxis
              type="number"
              tick={{ fill: "var(--ink-mute)", fontSize: 11 }}
              axisLine={{ stroke: "var(--line)" }}
              tickLine={false}
              tickFormatter={(v: number) => money(Math.round(v * 100))}
            />
            <YAxis
              type="category"
              dataKey="name"
              tick={{ fill: "var(--ink-2)", fontSize: 12 }}
              axisLine={false}
              tickLine={false}
              width={110}
            />
            <Bar dataKey="value" radius={[0, 5, 5, 0]} maxBarSize={22}>
              {data.map((entry) => (
                <Cell key={entry.name} fill={entry.isCurrent ? "var(--accent)" : "var(--ink-faint)"} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>
      <div className="row-sm" style={{ justifyContent: "space-between", marginTop: 4 }}>
        <span className="mono" style={{ fontSize: 12, color: "var(--ink-mute)" }}>{prior.label}: {money(prior.amountCents)}</span>
        <span className="mono" style={{ fontSize: 12, color: "var(--ink)" }}>{current.label}: {money(current.amountCents)}</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Run the test again**

Run: `cd ui && npx vitest run src/components/copilot/charts/FinSightChart.test.tsx`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add ui/package.json ui/package-lock.json ui/src/components/copilot/charts/
git commit -m "feat(copilot): add Recharts + themed FinSightBarComparison wrapper"
```

### Task B2: Verify the chart mid-stream before building further cards on it

**Files:**
- Test: `ui/src/components/copilot/charts/FinSightChart.stream.test.tsx`

This is the risk-flag check from the spec: confirm `FinSightBarComparison` behaves correctly when its parent re-renders rapidly (simulating a still-streaming message bubble), rather than deferring this discovery to when 6 more cards depend on the same pattern.

- [ ] **Step 1: Write the test**

```tsx
// ui/src/components/copilot/charts/FinSightChart.stream.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { FinSightBarComparison } from "./FinSightChart";

describe("FinSightBarComparison mid-stream stability", () => {
  it("still renders final labeled values correctly after several rapid re-renders (simulated streaming reflow)", () => {
    const { rerender } = render(
      <FinSightBarComparison
        title="Dining"
        current={{ label: "May", amountCents: 10000 }}
        prior={{ label: "Apr", amountCents: 8000 }}
      />
    );
    // Simulate the parent message bubble reflowing repeatedly while text streams in.
    for (let i = 0; i < 10; i++) {
      act(() => {
        rerender(
          <FinSightBarComparison
            title="Dining"
            current={{ label: "May", amountCents: 10000 }}
            prior={{ label: "Apr", amountCents: 8000 }}
          />
        );
      });
    }
    expect(screen.getByText("May: $100")).toBeInTheDocument();
    expect(screen.getByText("Apr: $80")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run it**

Run: `cd ui && npx vitest run src/components/copilot/charts/FinSightChart.stream.test.tsx`
Expected: PASS. jsdom's `ResponsiveContainer` reports width 0 by default in tests (no real layout engine), so this test exercises the "container never measures" path — if Recharts throws or the bar values stop matching after repeated re-renders, that's the exact failure mode the risk flag warned about, and `FinSightBarComparison` would need a manual-measurement fallback (matching `NetWorthChart.tsx`'s `ResizeObserver` pattern) before Phase C proceeds.

- [ ] **Step 3: If it fails** — do not proceed to Phase C. Instead, replace `ResponsiveContainer` in `FinSightChart.tsx` with the same manual-measurement pattern used in `ui/src/components/NetWorthChart.tsx` (a `useRef` + `ResizeObserver` reporting `clientWidth`, falling back to a fixed width in jsdom), re-run this test, and only continue once it passes.

- [ ] **Step 4: Commit** (only if Step 2 passed without changes)

```bash
git add ui/src/components/copilot/charts/FinSightChart.stream.test.tsx
git commit -m "test(copilot): verify FinSightBarComparison is stable across rapid re-renders"
```

---

## Phase C: Six remaining `AgentResponseBlock` kinds

Each kind touches the same three files every time (the "three-gate chain" from the spec): the Rust enum + first-pass validation (`crates/finsight-app/src/commands/agent.rs`), the artifact-emission bounds check (`crates/finsight-app/src/commands/copilot_chat.rs`), and the TS Zod schema (`ui/src/components/copilot/agUi/artifacts.ts`) — plus a new frontend renderer component. `TransactionTable` is done first since a stub Zod schema for it already exists; the pattern it establishes is then repeated for the other five with full code each time (per the "no placeholders" rule — later tasks are not allowed to say "same as Task C1").

### Task C0: Shared card primitives (`SegmentBar`, `ConfidenceBadge`)

**Files:**
- Create: `ui/src/components/copilot/cards/shared.tsx`
- Test: `ui/src/components/copilot/cards/shared.test.tsx`

Per the approved architecture, avoid duplicating visual primitives across the 7 card kinds. `AllocationSplitCard` (Task C4) and `CategoryBreakdownCard` (Task C3) both need a labeled, colored, proportional-width bar with a trailing amount — that's `SegmentBar`. `RecategorizationPreviewCard` (Task E1) needs a confidence meter — that's `ConfidenceBadge`. Both cards are built against these shared components below rather than duplicating the markup inline.

- [ ] **Step 1: Write the failing tests**

```tsx
// ui/src/components/copilot/cards/shared.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { SegmentBar, ConfidenceBadge } from "./shared";

describe("SegmentBar", () => {
  it("renders the label, a proportional-width fill, and the formatted amount", () => {
    render(<SegmentBar label="Housing" amountCents={185_000} maxCents={200_000} color="#A78BFA" />);
    expect(screen.getByText("Housing")).toBeInTheDocument();
    expect(screen.getByText("$1,850")).toBeInTheDocument();
    const fill = screen.getByTestId("segment-bar-fill");
    expect(fill.style.width).toBe("92.5%");
    expect(fill.style.background).toContain("171, 139, 250"); // #A78BFA in rgb, sanity-checks the color prop reached the DOM
  });

  it("renders an optional tag chip", () => {
    render(<SegmentBar label="Dining" amountCents={41_200} maxCents={100_000} color="#FB923C" tag={{ text: "lever" }} />);
    expect(screen.getByText("lever")).toBeInTheDocument();
  });

  it("renders a muted tag distinctly (e.g. 'fixed' vs 'lever')", () => {
    render(<SegmentBar label="Housing" amountCents={185_000} maxCents={200_000} color="#A78BFA" tag={{ text: "fixed", muted: true }} />);
    expect(screen.getByText("fixed").className).toContain("muted");
  });
});

describe("ConfidenceBadge", () => {
  it("renders a percentage-filled track and the numeric percentage", () => {
    render(<ConfidenceBadge confidence={0.99} color="#34D399" />);
    expect(screen.getByText("99%")).toBeInTheDocument();
    const fill = screen.getByTestId("confidence-fill");
    expect(fill.style.width).toBe("99%");
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd ui && npx vitest run src/components/copilot/cards/shared.test.tsx`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement**

```tsx
// ui/src/components/copilot/cards/shared.tsx
import { money } from "../../../utils/format";

/**
 * A labeled, colored, proportional-width bar with a trailing amount — shared
 * by AllocationSplitCard and CategoryBreakdownCard so their nearly-identical
 * "amount as a bar" visual isn't duplicated per card.
 */
export function SegmentBar({
  label,
  amountCents,
  maxCents,
  color,
  tag,
  dimmed,
}: {
  label: string;
  amountCents: number;
  maxCents: number;
  color: string;
  tag?: { text: string; muted?: boolean };
  dimmed?: boolean;
}) {
  const pct = maxCents > 0 ? (amountCents / maxCents) * 100 : 0;
  return (
    <div className="cp-bar-row">
      <div className="cp-bar-label">
        <span className="cp-dot" style={{ background: color }} />
        {label}
        {tag && <span className={`cp-bar-tag ${tag.muted ? "muted" : ""}`}>{tag.text}</span>}
      </div>
      <div className="cp-bar-track">
        <div
          data-testid="segment-bar-fill"
          className="cp-bar-fill"
          style={{ width: `${pct}%`, background: color, opacity: dimmed ? 0.4 : 1 }}
        />
      </div>
      <span className="cp-bar-amt mono">{money(amountCents)}</span>
    </div>
  );
}

/** A confidence-percentage meter — shared wherever a proposed change carries a confidence score. */
export function ConfidenceBadge({ confidence, color }: { confidence: number; color: string }) {
  const pct = Math.round(confidence * 100);
  return (
    <div className="cp-conf">
      <div className="cp-conf-track">
        <div data-testid="confidence-fill" className="cp-conf-fill" style={{ width: `${pct}%`, background: color }} />
      </div>
      <span className="cp-conf-num mono">{pct}%</span>
    </div>
  );
}
```

- [ ] **Step 4: Run the tests again**

Run: `cd ui && npx vitest run src/components/copilot/cards/shared.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/copilot/cards/shared.tsx ui/src/components/copilot/cards/shared.test.tsx
git commit -m "feat(copilot): add shared SegmentBar/ConfidenceBadge card primitives"
```

### Task C1: `TransactionTable`

**Files:**
- Modify: `crates/finsight-app/src/commands/agent.rs` (enum + `valid_response_block`)
- Modify: `crates/finsight-app/src/commands/copilot_chat.rs` (`should_emit_response_block`, `response_block_within_artifact_bounds`)
- Modify: `ui/src/components/copilot/agUi/artifacts.ts` (reconcile the existing stub schema)
- Create: `ui/src/components/copilot/cards/TransactionTableCard.tsx`
- Test (Rust): `crates/finsight-app/src/commands/agent.rs` (inline `#[cfg(test)]`)
- Test (TS): `ui/src/components/copilot/cards/TransactionTableCard.test.tsx`

- [ ] **Step 1: Write the failing Rust test**

Add to the existing `#[cfg(test)] mod tests` block in `crates/finsight-app/src/commands/agent.rs` (find it via `grep -n "mod tests" crates/finsight-app/src/commands/agent.rs`):

```rust
#[test]
fn transaction_table_block_round_trips_through_json() {
    let block = AgentResponseBlock::TransactionTable(AgentTransactionTableBlock {
        count: 42,
        total_cents: 1_193_000,
        rows: vec![AgentTxRow {
            date: "2026-05-03".to_string(),
            merchant: "Bay Property · Rent".to_string(),
            category_key: "Housing".to_string(),
            amount_cents: 185_000,
            flag: None,
        }],
        more: 32,
    });
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["kind"], "transactionTable");
    let back: AgentResponseBlock = serde_json::from_value(json).unwrap();
    assert!(valid_response_block(&back));
}

#[test]
fn transaction_table_block_with_zero_rows_is_invalid() {
    let block = AgentResponseBlock::TransactionTable(AgentTransactionTableBlock {
        count: 0,
        total_cents: 0,
        rows: vec![],
        more: 0,
    });
    assert!(!valid_response_block(&block));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p finsight-app transaction_table_block --lib`
Expected: FAIL with a compile error — `AgentTransactionTableBlock`/`AgentTxRow`/`AgentResponseBlock::TransactionTable` don't exist yet.

- [ ] **Step 3: Add the struct + enum variant + validation**

In `crates/finsight-app/src/commands/agent.rs`, add after `AgentMetricBlock` (around line 504, before the `AgentResponseBlock` enum):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentTxRow {
    pub date: String,
    pub merchant: String,
    pub category_key: String,
    pub amount_cents: i64,
    pub flag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentTransactionTableBlock {
    pub count: i64,
    pub total_cents: i64,
    pub rows: Vec<AgentTxRow>,
    pub more: i64,
}
```

Add the variant to `AgentResponseBlock` (line 508-523):

```rust
pub enum AgentResponseBlock {
    Markdown {
        markdown: String,
    },
    Table(AgentTableBlock),
    BarChart(AgentChartBlock),
    LineChart(AgentChartBlock),
    MetricGrid {
        metrics: Vec<AgentMetricBlock>,
    },
    Callout {
        tone: String,
        title: Option<String>,
        body: String,
    },
    TransactionTable(AgentTransactionTableBlock),
}
```

Add a match arm to `valid_response_block` (line 590-608):

```rust
        AgentResponseBlock::TransactionTable(t) => {
            t.count >= 0
                && !t.rows.is_empty()
                && t.rows.len() <= 200
                && t.rows.iter().all(|r| !r.merchant.trim().is_empty() && !r.category_key.trim().is_empty())
        }
```

- [ ] **Step 4: Run the Rust test again**

Run: `cargo test -p finsight-app transaction_table_block --lib`
Expected: PASS.

- [ ] **Step 5: Wire the artifact-emission gates in `copilot_chat.rs`**

Add to `should_emit_response_block` (copilot_chat.rs:1093-1102):

```rust
        AgentResponseBlock::TransactionTable(_) => true,
```

Add to `response_block_within_artifact_bounds` (copilot_chat.rs:1126-1157):

```rust
        AgentResponseBlock::TransactionTable(t) => {
            t.rows.len() <= ARTIFACT_MAX_TABLE_ROWS
                && t.rows.iter().all(|r| {
                    label_ok(&r.merchant)
                        && label_ok(&r.category_key)
                        && opt_label_ok(&r.flag)
                        && r.date.len() <= ARTIFACT_MAX_LABEL
                })
        }
```

- [ ] **Step 6: Run the full Rust workspace test suite**

Run: `cargo test --workspace`
Expected: all pass — the new match arms are exhaustive (Rust's compiler enforces this for the enum), so no other `match` on `AgentResponseBlock` should be left unhandled; if `cargo check` reports a non-exhaustive match anywhere else, add the same `AgentResponseBlock::TransactionTable(_) => true,` (or the appropriate value for that match's purpose) there too.

- [ ] **Step 7: Regenerate TS bindings**

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: `ui/src/api/bindings.ts`'s `AgentResponseBlock` type gains a `{ kind: "transactionTable" } & AgentTransactionTableBlock` union member.

- [ ] **Step 8: Reconcile the existing stub Zod schema**

In `ui/src/components/copilot/agUi/artifacts.ts`, the stub `TransactionTablePropsSchema` (lines 62-69) is a placeholder for a *different*, simpler shape than what the backend now actually emits (it was a guess made before this feature existed). Remove the stub and add a proper `transactionTable` member to `CopilotResponseBlockSchema`'s discriminated union instead — since `TransactionTable` is a `CopilotResponseBlock` kind (rendered via `FinSightResponseBlock`), not a separate top-level component:

```ts
  z.object({
    kind: z.literal("transactionTable"),
    count: z.number().int().nonnegative(),
    totalCents: z.number().int(),
    rows: z
      .array(
        z.object({
          date: shortString,
          merchant: shortString,
          categoryKey: shortString,
          amountCents: z.number().int(),
          flag: shortString.nullable(),
        }),
      )
      .min(1)
      .max(MAX_TABLE_ROWS),
    more: z.number().int().nonnegative(),
  }),
```

(insert this as a new array element in `CopilotResponseBlockSchema`, alongside the existing `markdown`/`table`/`barChart`/`lineChart`/`metricGrid`/`callout` entries). Then delete the now-superseded `TransactionTablePropsSchema` export and its `TransactionTable: TransactionTablePropsSchema` entry in `COMPONENT_PROP_SCHEMAS` (lines 62-77) — `FinSightResponseBlock`'s existing schema already covers it via the discriminated union.

- [ ] **Step 9: Update `artifacts.test.ts`** for the removed stub

Run: `grep -n "TransactionTable" ui/src/components/copilot/agUi/artifacts.test.ts` — if any test references the old `TransactionTable` component-level schema (as opposed to the new `transactionTable` block kind), update it to construct a `FinSightResponseBlock` envelope with a `transactionTable` block instead, following the existing test file's pattern for other block kinds.

- [ ] **Step 10: Write the failing frontend renderer test**

```tsx
// ui/src/components/copilot/cards/TransactionTableCard.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { TransactionTableCard } from "./TransactionTableCard";

describe("TransactionTableCard", () => {
  it("renders row merchant, category, and formatted amount, plus a more-count footer", () => {
    render(
      <TransactionTableCard
        block={{
          kind: "transactionTable",
          count: 42,
          totalCents: 1_193_000,
          rows: [
            { date: "2026-05-03", merchant: "Bay Property · Rent", categoryKey: "Housing", amountCents: 185_000, flag: null },
            { date: "2026-05-10", merchant: "PG&E", categoryKey: "Utilities", amountCents: 22_000, flag: "2.1× avg" },
          ],
          more: 40,
        }}
      />
    );
    expect(screen.getByText("42 transactions")).toBeInTheDocument();
    expect(screen.getByText("Bay Property · Rent")).toBeInTheDocument();
    expect(screen.getByText("$1,850")).toBeInTheDocument();
    expect(screen.getByText("2.1× avg")).toBeInTheDocument();
    expect(screen.getByText(/\+ 40 more/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 11: Run it to verify it fails**

Run: `cd ui && npx vitest run src/components/copilot/cards/TransactionTableCard.test.tsx`
Expected: FAIL — module does not exist.

- [ ] **Step 12: Implement the card**

```tsx
// ui/src/components/copilot/cards/TransactionTableCard.tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";

type Block = Extract<CopilotResponseBlock, { kind: "transactionTable" }>;

export function TransactionTableCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <div className="cp-card-head">
        <div className="cp-card-title">{block.count} transactions</div>
        <div className="cp-card-sub">{money(block.totalCents)} total · top {block.rows.length} by size</div>
      </div>
      <div className="cp-tx">
        {block.rows.map((r, i) => (
          <div key={i} className="cp-tx-row">
            <span className="cp-tx-date mono">{r.date}</span>
            <div className="cp-tx-merchant">
              <span className="cp-dot" style={{ background: colorForCategoryLabel(r.categoryKey) ?? "var(--ink-faint)" }} />
              <span>{r.merchant}</span>
              {r.flag && <span className="cp-tx-flag">{r.flag}</span>}
            </div>
            <span className="cp-tx-cat">{r.categoryKey}</span>
            <span className="cp-tx-amt mono">{money(r.amountCents)}</span>
          </div>
        ))}
        {block.more > 0 && (
          <div className="cp-tx-more">+ {block.more} more · {money(block.totalCents)} total</div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 13: Register it in `renderers.tsx`**

In `ui/src/components/copilot/renderers.tsx`, import the new card and add a case to `FinSightResponseBlock`'s switch (currently lines 318-338):

```tsx
import { TransactionTableCard } from "./cards/TransactionTableCard";
// ...
    case "transactionTable":
      return <TransactionTableCard block={block} />;
```

- [ ] **Step 14: Run the frontend test again**

Run: `cd ui && npx vitest run src/components/copilot/cards/TransactionTableCard.test.tsx`
Expected: PASS.

- [ ] **Step 15: Add the shared card CSS**

Append to `ui/src/styles/copilot-shell.css` (from Task A1):

```css
/* ── Cards (shared frame + Transaction table) ────────────────────────── */
.cp-card { background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius-lg); padding: 20px 22px; }
.cp-card-head { margin-bottom: 16px; }
.cp-card-title { font-size: 14px; font-weight: 600; color: var(--ink); letter-spacing: -0.01em; }
.cp-card-sub { font-size: 11.5px; color: var(--ink-faint); font-family: var(--mono); margin-top: 4px; }
.cp-dot { width: 9px; height: 9px; border-radius: 999px; flex-shrink: 0; display: inline-block; }
.cp-tx { display: flex; flex-direction: column; }
.cp-tx-row { display: grid; grid-template-columns: 60px 1fr auto auto; gap: 12px; align-items: center; padding: 10px 0; border-bottom: 1px solid var(--hairline); }
.cp-tx-row:first-child { padding-top: 0; }
.cp-tx-date { font-family: var(--mono); font-size: 11.5px; color: var(--ink-faint); }
.cp-tx-merchant { display: flex; align-items: center; gap: 9px; font-size: 13.5px; color: var(--ink); min-width: 0; }
.cp-tx-flag { font-family: var(--mono); font-size: 9.5px; background: var(--negative-2); color: var(--negative); padding: 1px 6px; border-radius: 4px; }
.cp-tx-cat { font-family: var(--mono); font-size: 11.5px; color: var(--ink-mute); }
.cp-tx-amt { font-family: var(--mono); font-size: 14px; color: var(--ink); font-variant-numeric: tabular-nums; }
.cp-tx-more { padding: 12px 0 0; font-size: 12px; color: var(--ink-mute); font-family: var(--mono); }
```

- [ ] **Step 16: Update the system prompt's supported-blocks list**

In `crates/finsight-agent/src/reasoning/engine/mod.rs`, extend the `Supported response_blocks are exactly: ...` sentence (line 211) to include:

```
, {{"kind":"transactionTable","count":42,"totalCents":1193000,"rows":[{{"date":"2026-05-03","merchant":"...","categoryKey":"...","amountCents":185000,"flag":null}}],"more":32}}
```

and extend the guidance sentence (line 212) — `Use metricGrid for ... table for alternatives/debt payoff/transaction review rows ...` — to add: `and transactionTable specifically for search_transactions results (never the generic table kind for those).`

- [ ] **Step 17: Full regression check**

Run: `cargo test --workspace && cd ui && npx vitest run && npx tsc --noEmit`
Expected: all green.

- [ ] **Step 18: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs crates/finsight-agent/src/reasoning/engine/mod.rs ui/src/api/bindings.ts ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/agUi/artifacts.test.ts ui/src/components/copilot/cards/TransactionTableCard.tsx ui/src/components/copilot/cards/TransactionTableCard.test.tsx ui/src/components/copilot/renderers.tsx ui/src/styles/copilot-shell.css
git commit -m "feat(copilot): add TransactionTable artifact kind end to end"
```

### Task C2: `AffordabilityVerdict`

**Files:** same three-gate files as C1, plus:
- Create: `ui/src/components/copilot/cards/AffordabilityVerdictCard.tsx`
- Test: `ui/src/components/copilot/cards/AffordabilityVerdictCard.test.tsx`

- [ ] **Step 1: Rust test** — add to `agent.rs`'s test module:

```rust
#[test]
fn affordability_verdict_round_trips_and_validates() {
    let block = AgentResponseBlock::AffordabilityVerdict(AgentAffordabilityVerdictBlock {
        can_afford: true,
        headline: "Yes".to_string(),
        sub: "$540 · about 1% of liquid cash".to_string(),
        caveat: Some("Exceeds your May Shopping envelope by $426.".to_string()),
        funding_source: Some(AgentFundingSource {
            label: "Cover it from Travel".to_string(),
            detail: "$500 budgeted · $0 spent".to_string(),
        }),
    });
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["kind"], "affordabilityVerdict");
    let back: AgentResponseBlock = serde_json::from_value(json).unwrap();
    assert!(valid_response_block(&back));
}

#[test]
fn affordability_verdict_with_empty_headline_is_invalid() {
    let block = AgentResponseBlock::AffordabilityVerdict(AgentAffordabilityVerdictBlock {
        can_afford: false,
        headline: "".to_string(),
        sub: "".to_string(),
        caveat: None,
        funding_source: None,
    });
    assert!(!valid_response_block(&block));
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p finsight-app affordability_verdict --lib` → FAIL, types missing.

- [ ] **Step 3: Implement in `agent.rs`** — add structs before the enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentFundingSource {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentAffordabilityVerdictBlock {
    pub can_afford: bool,
    pub headline: String,
    pub sub: String,
    pub caveat: Option<String>,
    pub funding_source: Option<AgentFundingSource>,
}
```

Add variant: `AffordabilityVerdict(AgentAffordabilityVerdictBlock),` to the enum. Add validation arm:

```rust
        AgentResponseBlock::AffordabilityVerdict(v) => {
            !v.headline.trim().is_empty() && !v.sub.trim().is_empty()
        }
```

- [ ] **Step 4: Run Rust test again** — PASS.

- [ ] **Step 5: `copilot_chat.rs` gates** — `should_emit_response_block`: `AgentResponseBlock::AffordabilityVerdict(_) => true,`. `response_block_within_artifact_bounds`:

```rust
        AgentResponseBlock::AffordabilityVerdict(v) => {
            label_ok(&v.headline)
                && label_ok(&v.sub)
                && v.caveat.chars_ok_or_none()
        }
```

Note: `Option<String>` doesn't have a `.chars_ok_or_none()` method — use the existing `opt_label_ok` helper instead:

```rust
        AgentResponseBlock::AffordabilityVerdict(v) => {
            label_ok(&v.headline)
                && label_ok(&v.sub)
                && opt_label_ok(&v.caveat)
                && v.funding_source.as_ref().is_none_or(|f| label_ok(&f.label) && label_ok(&f.detail))
        }
```

- [ ] **Step 6: Full Rust suite** — `cargo test --workspace` → PASS.

- [ ] **Step 7: Regenerate bindings** — `cargo run -p finsight-tauri --bin export_bindings`.

- [ ] **Step 8: Extend the Zod schema** in `artifacts.ts`'s `CopilotResponseBlockSchema`:

```ts
  z.object({
    kind: z.literal("affordabilityVerdict"),
    canAfford: z.boolean(),
    headline: shortString,
    sub: shortString,
    caveat: shortString.nullable(),
    fundingSource: z.object({ label: shortString, detail: shortString }).nullable(),
  }),
```

- [ ] **Step 9: Frontend test** —

```tsx
// ui/src/components/copilot/cards/AffordabilityVerdictCard.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { AffordabilityVerdictCard } from "./AffordabilityVerdictCard";

describe("AffordabilityVerdictCard", () => {
  it("renders the headline, sub, caveat, and funding source", () => {
    render(
      <AffordabilityVerdictCard
        block={{
          kind: "affordabilityVerdict",
          canAfford: true,
          headline: "Yes",
          sub: "$540 · about 1% of liquid cash · 0 goals affected",
          caveat: "Exceeds your May Shopping envelope by $426.",
          fundingSource: { label: "Cover it from Travel", detail: "$500 budgeted · $0 spent" },
        }}
      />
    );
    expect(screen.getByText("Yes")).toBeInTheDocument();
    expect(screen.getByText(/1% of liquid cash/)).toBeInTheDocument();
    expect(screen.getByText(/Exceeds your May Shopping envelope/)).toBeInTheDocument();
    expect(screen.getByText("Cover it from Travel")).toBeInTheDocument();
  });

  it("omits the caveat and funding rows when absent", () => {
    render(
      <AffordabilityVerdictCard
        block={{ kind: "affordabilityVerdict", canAfford: false, headline: "No", sub: "Not enough liquid cash", caveat: null, fundingSource: null }}
      />
    );
    expect(screen.getByText("No")).toBeInTheDocument();
    expect(screen.queryByText(/Cover it from/)).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 10: Run to verify failure** — FAIL, module missing.

- [ ] **Step 11: Implement** —

```tsx
// ui/src/components/copilot/cards/AffordabilityVerdictCard.tsx
import type { CopilotResponseBlock } from "../../../api/client";
import * as I from "../../Icons";

type Block = Extract<CopilotResponseBlock, { kind: "affordabilityVerdict" }>;

export function AffordabilityVerdictCard({ block }: { block: Block }) {
  return (
    <div className="cp-card" style={{ overflow: "hidden" }}>
      <div className="cp-verdict-hero">
        <div className={`cp-verdict-big ${block.canAfford ? "pos" : "neg"}`}>{block.headline}</div>
        <div className="cp-verdict-sub">{block.sub}</div>
      </div>
      {block.caveat && (
        <div className="cp-caveat">
          <I.Bolt width={13} height={13} />
          <span>{block.caveat}</span>
        </div>
      )}
      {block.fundingSource && (
        <div className="cp-fund">
          <div className="cp-fund-label">{block.fundingSource.label}</div>
          <div className="cp-fund-detail mono">{block.fundingSource.detail}</div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 12: Register in `renderers.tsx`** — import + `case "affordabilityVerdict": return <AffordabilityVerdictCard block={block} />;`

- [ ] **Step 13: Run frontend test again** — PASS.

- [ ] **Step 14: Add CSS** to `copilot-shell.css`:

```css
.cp-verdict-hero { display: flex; align-items: baseline; gap: 16px; padding: 8px 0 18px; }
.cp-verdict-big { font-size: 64px; font-weight: 600; line-height: 1; letter-spacing: -0.04em; }
.cp-verdict-big.pos { color: var(--accent); }
.cp-verdict-big.neg { color: var(--negative); }
.cp-verdict-sub { font-size: 14px; color: var(--ink-mute); }
.cp-caveat { display: flex; align-items: flex-start; gap: 9px; padding: 12px 14px; border-radius: 9px; background: var(--warning-2); color: var(--warning); font-size: 13px; line-height: 1.5; }
.cp-caveat svg { margin-top: 2px; flex-shrink: 0; }
.cp-fund { margin-top: 10px; padding: 12px 14px; border: 1px solid var(--line); border-radius: 9px; background: var(--bg-2); }
.cp-fund-label { font-size: 13.5px; font-weight: 500; color: var(--ink); }
.cp-fund-detail { font-size: 11.5px; color: var(--ink-mute); margin-top: 5px; font-family: var(--mono); }
```

- [ ] **Step 15: Update the system prompt** in `engine/mod.rs` line 211-212 (append to the supported blocks list and the guidance sentence: `and affordabilityVerdict for run_purchase_affordability results`).

- [ ] **Step 16: Full regression** — `cargo test --workspace && cd ui && npx vitest run && npx tsc --noEmit` → all green.

- [ ] **Step 17: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs crates/finsight-agent/src/reasoning/engine/mod.rs ui/src/api/bindings.ts ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/cards/AffordabilityVerdictCard.tsx ui/src/components/copilot/cards/AffordabilityVerdictCard.test.tsx ui/src/components/copilot/renderers.tsx ui/src/styles/copilot-shell.css
git commit -m "feat(copilot): add AffordabilityVerdict artifact kind end to end"
```

### Task C3: `CategoryBreakdown`

**Files:** same pattern as C1/C2.

- [ ] **Step 1: Rust test** —

```rust
#[test]
fn category_breakdown_round_trips_and_validates() {
    let block = AgentResponseBlock::CategoryBreakdown(AgentCategoryBreakdownBlock {
        period_label: "May".to_string(),
        rows: vec![
            AgentCategoryRow { category_key: "Housing".to_string(), amount_cents: 185_000, is_fixed: true, is_lever: false },
            AgentCategoryRow { category_key: "Dining".to_string(), amount_cents: 41_200, is_fixed: false, is_lever: true },
        ],
    });
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["kind"], "categoryBreakdown");
    let back: AgentResponseBlock = serde_json::from_value(json).unwrap();
    assert!(valid_response_block(&back));
}

#[test]
fn category_breakdown_with_no_rows_is_invalid() {
    let block = AgentResponseBlock::CategoryBreakdown(AgentCategoryBreakdownBlock {
        period_label: "May".to_string(),
        rows: vec![],
    });
    assert!(!valid_response_block(&block));
}
```

- [ ] **Step 2: Run, verify failure.**

- [ ] **Step 3: Implement in `agent.rs`** —

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentCategoryRow {
    pub category_key: String,
    pub amount_cents: i64,
    pub is_fixed: bool,
    pub is_lever: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentCategoryBreakdownBlock {
    pub period_label: String,
    pub rows: Vec<AgentCategoryRow>,
}
```

Enum variant: `CategoryBreakdown(AgentCategoryBreakdownBlock),`. Validation:

```rust
        AgentResponseBlock::CategoryBreakdown(b) => {
            !b.period_label.trim().is_empty()
                && !b.rows.is_empty()
                && b.rows.len() <= 30
                && b.rows.iter().all(|r| !r.category_key.trim().is_empty())
        }
```

- [ ] **Step 4: Run Rust test again** — PASS.

- [ ] **Step 5: `copilot_chat.rs` gates** —

`should_emit_response_block`: `AgentResponseBlock::CategoryBreakdown(_) => true,`

`response_block_within_artifact_bounds`:

```rust
        AgentResponseBlock::CategoryBreakdown(b) => {
            label_ok(&b.period_label)
                && b.rows.len() <= 30
                && b.rows.iter().all(|r| label_ok(&r.category_key))
        }
```

- [ ] **Step 6: Full Rust suite, regenerate bindings** (same commands as C1/C2).

- [ ] **Step 7: Zod schema addition** —

```ts
  z.object({
    kind: z.literal("categoryBreakdown"),
    periodLabel: shortString,
    rows: z
      .array(z.object({ categoryKey: shortString, amountCents: z.number().int(), isFixed: z.boolean(), isLever: z.boolean() }))
      .min(1)
      .max(30),
  }),
```

- [ ] **Step 8: Frontend test** —

```tsx
// ui/src/components/copilot/cards/CategoryBreakdownCard.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { CategoryBreakdownCard } from "./CategoryBreakdownCard";

describe("CategoryBreakdownCard", () => {
  it("renders each row's category and amount, tagging fixed and lever rows", () => {
    render(
      <CategoryBreakdownCard
        block={{
          kind: "categoryBreakdown",
          periodLabel: "May",
          rows: [
            { categoryKey: "Housing", amountCents: 185_000, isFixed: true, isLever: false },
            { categoryKey: "Dining", amountCents: 41_200, isFixed: false, isLever: true },
          ],
        }}
      />
    );
    expect(screen.getByText("Housing")).toBeInTheDocument();
    expect(screen.getByText("$1,850")).toBeInTheDocument();
    expect(screen.getByText("fixed")).toBeInTheDocument();
    expect(screen.getByText("lever")).toBeInTheDocument();
  });
});
```

- [ ] **Step 9: Run, verify failure.**

- [ ] **Step 10: Implement** —

```tsx
// ui/src/components/copilot/cards/CategoryBreakdownCard.tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import { SegmentBar } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "categoryBreakdown" }>;

export function CategoryBreakdownCard({ block }: { block: Block }) {
  const max = Math.max(...block.rows.map((r) => r.amountCents));
  return (
    <div className="cp-card">
      <div className="cp-card-head">
        <div className="cp-card-title">Spending by category · {block.periodLabel}</div>
        <div className="cp-card-sub">● fixed cost · ◆ the lever</div>
      </div>
      <div className="cp-bars">
        {block.rows.map((r) => (
          <SegmentBar
            key={r.categoryKey}
            label={r.categoryKey}
            amountCents={r.amountCents}
            maxCents={max}
            color={colorForCategoryLabel(r.categoryKey) ?? "var(--ink-faint)"}
            tag={r.isLever ? { text: "lever" } : r.isFixed ? { text: "fixed", muted: true } : undefined}
            dimmed={r.isFixed}
          />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 11: Register in `renderers.tsx`** — `case "categoryBreakdown": return <CategoryBreakdownCard block={block} />;`

- [ ] **Step 12: Run frontend test again** — PASS.

- [ ] **Step 13: CSS** —

```css
.cp-bars { display: flex; flex-direction: column; gap: 11px; }
.cp-bar-row { display: grid; grid-template-columns: 140px 1fr 64px; gap: 12px; align-items: center; }
.cp-bar-label { display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--ink-2); }
.cp-bar-row.is-lever .cp-bar-label { color: var(--ink); font-weight: 500; }
.cp-bar-tag { font-family: var(--mono); font-size: 9px; letter-spacing: 0.05em; text-transform: uppercase; color: var(--c-dining); border: 1px solid currentColor; border-radius: 4px; padding: 1px 5px; }
.cp-bar-tag.muted { color: var(--ink-faint); }
.cp-bar-track { height: 8px; background: var(--bg-2); border-radius: 999px; overflow: hidden; }
.cp-bar-fill { height: 100%; border-radius: 999px; }
.cp-bar-amt { font-family: var(--mono); font-size: 12.5px; color: var(--ink); text-align: right; font-variant-numeric: tabular-nums; }
```

- [ ] **Step 14: Update system prompt** (`engine/mod.rs`, append `categoryBreakdown` example + guidance).

- [ ] **Step 15: Full regression, commit** (same as prior tasks — file list adds `CategoryBreakdownCard.tsx`/`.test.tsx`).

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs crates/finsight-agent/src/reasoning/engine/mod.rs ui/src/api/bindings.ts ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/cards/CategoryBreakdownCard.tsx ui/src/components/copilot/cards/CategoryBreakdownCard.test.tsx ui/src/components/copilot/renderers.tsx ui/src/styles/copilot-shell.css
git commit -m "feat(copilot): add CategoryBreakdown artifact kind end to end"
```

### Task C4: `AllocationSplit`

**Files:** same pattern.

- [ ] **Step 1: Rust test** —

```rust
#[test]
fn allocation_split_round_trips_and_validates() {
    let block = AgentResponseBlock::AllocationSplit(AgentAllocationSplitBlock {
        total_cents: 520_000,
        segments: vec![
            AgentAllocationSegment { label: "Pay off Amex".to_string(), amount_cents: 241_800, rationale: "24.9% APR".to_string(), category_key: "debt".to_string() },
            AgentAllocationSegment { label: "Emergency fund".to_string(), amount_cents: 180_000, rationale: "76% to target".to_string(), category_key: "savings".to_string() },
        ],
    });
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["kind"], "allocationSplit");
    let back: AgentResponseBlock = serde_json::from_value(json).unwrap();
    assert!(valid_response_block(&back));
}

#[test]
fn allocation_split_with_zero_total_is_invalid() {
    let block = AgentResponseBlock::AllocationSplit(AgentAllocationSplitBlock { total_cents: 0, segments: vec![] });
    assert!(!valid_response_block(&block));
}
```

- [ ] **Step 2: Run, verify failure.**

- [ ] **Step 3: Implement in `agent.rs`** —

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentAllocationSegment {
    pub label: String,
    pub amount_cents: i64,
    pub rationale: String,
    pub category_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentAllocationSplitBlock {
    pub total_cents: i64,
    pub segments: Vec<AgentAllocationSegment>,
}
```

Enum variant: `AllocationSplit(AgentAllocationSplitBlock),`. Validation:

```rust
        AgentResponseBlock::AllocationSplit(b) => {
            b.total_cents > 0
                && !b.segments.is_empty()
                && b.segments.len() <= 12
                && b.segments.iter().all(|s| !s.label.trim().is_empty())
        }
```

- [ ] **Step 4: Run Rust test again** — PASS.

- [ ] **Step 5: `copilot_chat.rs` gates** — `should_emit_response_block`: `AgentResponseBlock::AllocationSplit(_) => true,`. Bounds:

```rust
        AgentResponseBlock::AllocationSplit(b) => {
            b.segments.len() <= 12
                && b.segments.iter().all(|s| label_ok(&s.label) && label_ok(&s.rationale) && label_ok(&s.category_key))
        }
```

- [ ] **Step 6: Full Rust suite, regenerate bindings.**

- [ ] **Step 7: Zod schema** —

```ts
  z.object({
    kind: z.literal("allocationSplit"),
    totalCents: z.number().int().positive(),
    segments: z
      .array(z.object({ label: shortString, amountCents: z.number().int(), rationale: shortString, categoryKey: shortString }))
      .min(1)
      .max(12),
  }),
```

- [ ] **Step 8: Frontend test** —

```tsx
// ui/src/components/copilot/cards/AllocationSplitCard.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { AllocationSplitCard } from "./AllocationSplitCard";

describe("AllocationSplitCard", () => {
  it("renders the total, each segment's label/rationale/amount, and a proportional bar", () => {
    render(
      <AllocationSplitCard
        block={{
          kind: "allocationSplit",
          totalCents: 520_000,
          segments: [
            { label: "Pay off Amex", amountCents: 241_800, rationale: "24.9% APR — guaranteed return", categoryKey: "debt" },
            { label: "Emergency fund", amountCents: 180_000, rationale: "76% ➜ 83% of target", categoryKey: "savings" },
          ],
        }}
      />
    );
    expect(screen.getByText(/Recommended split of \$5,200/)).toBeInTheDocument();
    expect(screen.getByText("Pay off Amex")).toBeInTheDocument();
    expect(screen.getByText("$2,418")).toBeInTheDocument();
    expect(screen.getByText("24.9% APR — guaranteed return")).toBeInTheDocument();
  });
});
```

- [ ] **Step 9: Run, verify failure.**

- [ ] **Step 10: Implement** —

```tsx
// ui/src/components/copilot/cards/AllocationSplitCard.tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";

type Block = Extract<CopilotResponseBlock, { kind: "allocationSplit" }>;

const FALLBACK_COLORS = ["var(--accent)", "var(--c-travel)", "var(--c-dining)", "var(--c-shopping)"];

export function AllocationSplitCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <div className="cp-card-head">
        <div className="cp-card-title">Recommended split of {money(block.totalCents)}</div>
        <div className="cp-card-sub">every dollar, accounted for</div>
      </div>
      <div className="cp-alloc-bar">
        {block.segments.map((s, i) => (
          <div
            key={s.label}
            className="cp-alloc-seg"
            style={{
              width: `${(s.amountCents / block.totalCents) * 100}%`,
              background: colorForCategoryLabel(s.categoryKey) ?? FALLBACK_COLORS[i % FALLBACK_COLORS.length],
            }}
            title={`${s.label} · ${money(s.amountCents)}`}
          />
        ))}
      </div>
      <div className="cp-alloc-legend">
        {block.segments.map((s, i) => {
          const color = colorForCategoryLabel(s.categoryKey) ?? FALLBACK_COLORS[i % FALLBACK_COLORS.length];
          return (
            <div key={s.label} className="cp-alloc-row">
              <span className="cp-dot" style={{ background: color }} />
              <div className="cp-alloc-meta">
                <span className="cp-alloc-label">{s.label}</span>
                <span className="cp-alloc-why">{s.rationale}</span>
              </div>
              <span className="cp-alloc-amt mono" style={{ color }}>{money(s.amountCents)}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

- [ ] **Step 11: Register in `renderers.tsx`** — `case "allocationSplit": return <AllocationSplitCard block={block} />;`

- [ ] **Step 12: Run frontend test again** — PASS.

- [ ] **Step 13: CSS** —

```css
.cp-alloc-bar { display: flex; height: 26px; border-radius: 7px; overflow: hidden; gap: 2px; background: var(--bg-2); }
.cp-alloc-seg { height: 100%; border-radius: 3px; }
.cp-alloc-legend { display: flex; flex-direction: column; margin-top: 16px; }
.cp-alloc-row { display: grid; grid-template-columns: 9px 1fr auto; gap: 12px; align-items: center; padding: 9px 0; border-bottom: 1px solid var(--hairline); }
.cp-alloc-row:last-child { border-bottom: 0; }
.cp-alloc-meta { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.cp-alloc-label { font-size: 14px; font-weight: 500; color: var(--ink); }
.cp-alloc-why { font-size: 12px; color: var(--ink-mute); }
.cp-alloc-amt { font-size: 16px; font-weight: 600; letter-spacing: -0.02em; font-family: var(--mono); }
```

- [ ] **Step 14: Update system prompt** (append `allocationSplit`, note it's typically produced from `analyze_cash_inflow`).

- [ ] **Step 15: Full regression, commit.**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs crates/finsight-agent/src/reasoning/engine/mod.rs ui/src/api/bindings.ts ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/cards/AllocationSplitCard.tsx ui/src/components/copilot/cards/AllocationSplitCard.test.tsx ui/src/components/copilot/renderers.tsx ui/src/styles/copilot-shell.css
git commit -m "feat(copilot): add AllocationSplit artifact kind end to end"
```

### Task C5: `RankedOptions`

**Files:** same pattern.

- [ ] **Step 1: Rust test** —

```rust
#[test]
fn ranked_options_round_trips_and_validates() {
    let block = AgentResponseBlock::RankedOptions(AgentRankedOptionsBlock {
        title: "The three routes you asked about".to_string(),
        options: vec![
            AgentRankedOption { rank_tone: "primary".to_string(), label: "Pay off the loan".to_string(), detail: "$2,418 → Amex Gold".to_string(), rationale: "Highest-interest debt at 24.9%.".to_string() },
        ],
    });
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["kind"], "rankedOptions");
    let back: AgentResponseBlock = serde_json::from_value(json).unwrap();
    assert!(valid_response_block(&back));
}

#[test]
fn ranked_options_with_no_options_is_invalid() {
    let block = AgentResponseBlock::RankedOptions(AgentRankedOptionsBlock { title: "Empty".to_string(), options: vec![] });
    assert!(!valid_response_block(&block));
}
```

- [ ] **Step 2: Run, verify failure.**

- [ ] **Step 3: Implement in `agent.rs`** —

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentRankedOption {
    pub rank_tone: String,
    pub label: String,
    pub detail: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentRankedOptionsBlock {
    pub title: String,
    pub options: Vec<AgentRankedOption>,
}
```

Enum variant: `RankedOptions(AgentRankedOptionsBlock),`. Validation:

```rust
        AgentResponseBlock::RankedOptions(b) => {
            !b.title.trim().is_empty()
                && !b.options.is_empty()
                && b.options.len() <= 10
                && b.options.iter().all(|o| !o.label.trim().is_empty() && matches!(o.rank_tone.as_str(), "primary" | "neutral" | "muted"))
        }
```

- [ ] **Step 4: Run Rust test again** — PASS.

- [ ] **Step 5: `copilot_chat.rs` gates** — `should_emit_response_block`: `AgentResponseBlock::RankedOptions(_) => true,`. Bounds:

```rust
        AgentResponseBlock::RankedOptions(b) => {
            label_ok(&b.title)
                && b.options.len() <= 10
                && b.options.iter().all(|o| label_ok(&o.label) && label_ok(&o.detail) && label_ok(&o.rationale))
        }
```

- [ ] **Step 6: Full Rust suite, regenerate bindings.**

- [ ] **Step 7: Zod schema** —

```ts
  z.object({
    kind: z.literal("rankedOptions"),
    title: shortString,
    options: z
      .array(z.object({ rankTone: z.enum(["primary", "neutral", "muted"]), label: shortString, detail: shortString, rationale: shortString }))
      .min(1)
      .max(10),
  }),
```

- [ ] **Step 8: Frontend test** —

```tsx
// ui/src/components/copilot/cards/RankedOptionsCard.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { RankedOptionsCard } from "./RankedOptionsCard";

describe("RankedOptionsCard", () => {
  it("renders the title and each option's verdict tone, label, detail, and rationale", () => {
    render(
      <RankedOptionsCard
        block={{
          kind: "rankedOptions",
          title: "The three routes you asked about",
          options: [
            { rankTone: "primary", label: "Pay off the loan", detail: "$2,418 → Amex Gold", rationale: "Highest-interest debt at 24.9%." },
            { rankTone: "muted", label: "Save for a car", detail: "no active goal", rationale: "Finish the emergency fund first." },
          ],
        }}
      />
    );
    expect(screen.getByText("The three routes you asked about")).toBeInTheDocument();
    expect(screen.getByText("Pay off the loan")).toBeInTheDocument();
    expect(screen.getByText("$2,418 → Amex Gold")).toBeInTheDocument();
    expect(screen.getByText("Do this first")).toBeInTheDocument();
    expect(screen.getByText("Not yet")).toBeInTheDocument();
  });
});
```

- [ ] **Step 9: Run, verify failure.**

- [ ] **Step 10: Implement** —

```tsx
// ui/src/components/copilot/cards/RankedOptionsCard.tsx
import type { CopilotResponseBlock } from "../../../api/client";

type Block = Extract<CopilotResponseBlock, { kind: "rankedOptions" }>;

const VERDICT_LABEL: Record<Block["options"][number]["rankTone"], string> = {
  primary: "Do this first",
  neutral: "With what's left",
  muted: "Not yet",
};

export function RankedOptionsCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <div className="cp-card-head">
        <div className="cp-card-title">{block.title}</div>
      </div>
      <div className="cp-options">
        {block.options.map((o, i) => (
          <div key={i} className={`cp-option ${o.rankTone === "primary" ? "is-primary" : ""}`}>
            <div className="cp-option-top">
              <span className={`cp-verdict cp-verdict-${o.rankTone}`}>{VERDICT_LABEL[o.rankTone]}</span>
              <span className="cp-option-detail mono">{o.detail}</span>
            </div>
            <div className="cp-option-label">{o.label}</div>
            <div className="cp-option-why">{o.rationale}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 11: Register in `renderers.tsx`** — `case "rankedOptions": return <RankedOptionsCard block={block} />;`

- [ ] **Step 12: Run frontend test again** — PASS.

- [ ] **Step 13: CSS** —

```css
.cp-options { display: flex; flex-direction: column; gap: 8px; }
.cp-option { border: 1px solid var(--line); border-radius: var(--radius); background: var(--bg-2); padding: 14px 16px; }
.cp-option.is-primary { border-color: var(--accent-3); background: linear-gradient(135deg, var(--accent-2), transparent 70%); }
.cp-option-top { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; }
.cp-verdict { font-family: var(--mono); font-size: 10px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; padding: 3px 8px; border-radius: 5px; }
.cp-verdict-primary { background: var(--accent); color: var(--accent-ink); }
.cp-verdict-neutral { background: var(--surface-2); color: var(--ink-2); border: 1px solid var(--line); }
.cp-verdict-muted { background: transparent; color: var(--ink-faint); border: 1px solid var(--line); }
.cp-option-detail { margin-left: auto; font-size: 12px; color: var(--ink-mute); font-family: var(--mono); }
.cp-option-label { font-size: 15px; font-weight: 600; color: var(--ink); letter-spacing: -0.01em; }
.cp-option-why { font-size: 13px; color: var(--ink-mute); margin-top: 4px; line-height: 1.5; }
```

- [ ] **Step 14: Update system prompt** (append `rankedOptions`).

- [ ] **Step 15: Full regression, commit.**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs crates/finsight-agent/src/reasoning/engine/mod.rs ui/src/api/bindings.ts ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/cards/RankedOptionsCard.tsx ui/src/components/copilot/cards/RankedOptionsCard.test.tsx ui/src/components/copilot/renderers.tsx ui/src/styles/copilot-shell.css
git commit -m "feat(copilot): add RankedOptions artifact kind end to end"
```

### Task C6: `ComparisonBars` (wires up Phase B's `FinSightBarComparison`)

**Files:** same pattern, plus wiring to `FinSightBarComparison` from Task B1.

- [ ] **Step 1: Rust test** —

```rust
#[test]
fn comparison_bars_round_trips_and_validates() {
    let block = AgentResponseBlock::ComparisonBars(AgentComparisonBarsBlock {
        title: "Dining · this month vs average".to_string(),
        current: AgentMoneyPoint { label: "May 2026".to_string(), amount_cents: 41_200 },
        prior: AgentMoneyPoint { label: "12-mo avg".to_string(), amount_cents: 36_500 },
    });
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["kind"], "comparisonBars");
    let back: AgentResponseBlock = serde_json::from_value(json).unwrap();
    assert!(valid_response_block(&back));
}

#[test]
fn comparison_bars_with_empty_title_is_invalid() {
    let block = AgentResponseBlock::ComparisonBars(AgentComparisonBarsBlock {
        title: "".to_string(),
        current: AgentMoneyPoint { label: "May".to_string(), amount_cents: 100 },
        prior: AgentMoneyPoint { label: "Apr".to_string(), amount_cents: 80 },
    });
    assert!(!valid_response_block(&block));
}
```

- [ ] **Step 2: Run, verify failure.**

- [ ] **Step 3: Implement in `agent.rs`** —

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentMoneyPoint {
    pub label: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentComparisonBarsBlock {
    pub title: String,
    pub current: AgentMoneyPoint,
    pub prior: AgentMoneyPoint,
}
```

Enum variant: `ComparisonBars(AgentComparisonBarsBlock),`. Validation:

```rust
        AgentResponseBlock::ComparisonBars(b) => {
            !b.title.trim().is_empty() && !b.current.label.trim().is_empty() && !b.prior.label.trim().is_empty()
        }
```

- [ ] **Step 4: Run Rust test again** — PASS.

- [ ] **Step 5: `copilot_chat.rs` gates** — `should_emit_response_block`: `AgentResponseBlock::ComparisonBars(_) => true,`. Bounds:

```rust
        AgentResponseBlock::ComparisonBars(b) => {
            label_ok(&b.title) && label_ok(&b.current.label) && label_ok(&b.prior.label)
        }
```

- [ ] **Step 6: Full Rust suite, regenerate bindings.**

- [ ] **Step 7: Zod schema** —

```ts
  z.object({
    kind: z.literal("comparisonBars"),
    title: shortString,
    current: z.object({ label: shortString, amountCents: z.number().int() }),
    prior: z.object({ label: shortString, amountCents: z.number().int() }),
  }),
```

- [ ] **Step 8: Frontend test** —

```tsx
// ui/src/components/copilot/cards/ComparisonBarsCard.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ComparisonBarsCard } from "./ComparisonBarsCard";

describe("ComparisonBarsCard", () => {
  it("delegates to FinSightBarComparison with the block's title/current/prior, only once streaming has finished", () => {
    render(
      <ComparisonBarsCard
        isRunning={false}
        block={{
          kind: "comparisonBars",
          title: "Dining · this month vs average",
          current: { label: "May 2026", amountCents: 41_200 },
          prior: { label: "12-mo avg", amountCents: 36_500 },
        }}
      />
    );
    expect(screen.getByText("Dining · this month vs average")).toBeInTheDocument();
    expect(screen.getByText("May 2026: $412")).toBeInTheDocument();
  });

  it("shows a lightweight placeholder instead of mounting the chart while the message is still streaming", () => {
    render(
      <ComparisonBarsCard
        isRunning
        block={{
          kind: "comparisonBars",
          title: "Dining",
          current: { label: "May", amountCents: 100 },
          prior: { label: "Apr", amountCents: 80 },
        }}
      />
    );
    expect(screen.getByText("Dining")).toBeInTheDocument();
    expect(screen.queryByText("May: $1")).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 9: Run, verify failure.**

- [ ] **Step 10: Implement, wiring in the mid-stream guard from Phase B's risk flag** —

```tsx
// ui/src/components/copilot/cards/ComparisonBarsCard.tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { FinSightBarComparison } from "../charts/FinSightChart";

type Block = Extract<CopilotResponseBlock, { kind: "comparisonBars" }>;

/**
 * Recharts' ResponsiveContainer renders blank at width:0 and re-animates on
 * every reflow (see the FinSightChart.stream test in Phase B) — so this card
 * only mounts the chart once the assistant message has finished streaming,
 * matching the mockup's own reveal order where cards appear after the answer.
 */
export function ComparisonBarsCard({ block, isRunning }: { block: Block; isRunning: boolean }) {
  if (isRunning) {
    return (
      <div className="cp-card">
        <div className="cp-card-title">{block.title}</div>
        <p className="muted" style={{ fontSize: 12.5, marginTop: 8 }}>Preparing comparison…</p>
      </div>
    );
  }
  return (
    <FinSightBarComparison
      title={block.title}
      current={{ label: block.current.label, amountCents: block.current.amountCents }}
      prior={{ label: block.prior.label, amountCents: block.prior.amountCents }}
    />
  );
}
```

- [ ] **Step 11: Register in `renderers.tsx`**, threading the running state through. `FinSightResponseBlock` (renderers.tsx:318-338) needs an `isRunning` prop now, so its call site in `CopilotToolCard` (line 84-86) must pass `status.type === "running"`:

```tsx
    if (block) {
      return <FinSightResponseBlock block={block} isRunning={status.type === "running"} />;
    }
```

Update `FinSightResponseBlock`'s signature and add the case:

```tsx
export function FinSightResponseBlock({ block, isRunning }: { block: CopilotResponseBlock; isRunning: boolean }) {
  switch (block.kind) {
    // ...existing cases unchanged...
    case "comparisonBars":
      return <ComparisonBarsCard block={block} isRunning={isRunning} />;
```

- [ ] **Step 12: Run frontend test again** — PASS.

- [ ] **Step 13: Update system prompt** (append `comparisonBars`).

- [ ] **Step 14: Full regression, commit.**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs crates/finsight-agent/src/reasoning/engine/mod.rs ui/src/api/bindings.ts ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/cards/ComparisonBarsCard.tsx ui/src/components/copilot/cards/ComparisonBarsCard.test.tsx ui/src/components/copilot/renderers.tsx
git commit -m "feat(copilot): add ComparisonBars artifact kind, wired to FinSightBarComparison"
```

### Task C7: Restyle the existing 6 generic block kinds

**Files:**
- Modify: `ui/src/components/copilot/renderers.tsx` (`TableBlock`, `ChartBlock`, `MetricGrid`, `CalloutBlock`)
- Test: existing tests for these must still pass; add coverage if none exists

- [ ] **Step 1: Check for existing coverage**

Run: `cd ui && grep -rn "TableBlock\|MetricGrid\|CalloutBlock" src/components/copilot/*.test.tsx 2>/dev/null` — if nothing is found, write `ui/src/components/copilot/renderers.test.tsx` covering all 4 before restyling (regression safety net):

```tsx
// ui/src/components/copilot/renderers.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { FinSightResponseBlock } from "./renderers";

describe("FinSightResponseBlock — existing generic kinds", () => {
  it("renders a table block's rows", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "table", title: "Alternatives", columns: ["Option", "Cost"], rows: [["A", "$10"]] }}
      />
    );
    expect(screen.getByText("Alternatives")).toBeInTheDocument();
    expect(screen.getByText("A")).toBeInTheDocument();
  });

  it("renders a metricGrid block's metrics", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "metricGrid", metrics: [{ label: "Net worth", value: "$20,606", detail: null, tone: null }] }}
      />
    );
    expect(screen.getByText("Net worth")).toBeInTheDocument();
    expect(screen.getByText("$20,606")).toBeInTheDocument();
  });

  it("renders a callout block's title and body", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "callout", tone: "warning", title: "Heads up", body: "Missing APR data." }}
      />
    );
    expect(screen.getByText("Heads up")).toBeInTheDocument();
    expect(screen.getByText("Missing APR data.")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run it to confirm current pass (or write-then-fail if genuinely new)**

Run: `cd ui && npx vitest run src/components/copilot/renderers.test.tsx`
Expected: PASS against the current implementation (this is a safety net, not new behavior yet).

- [ ] **Step 3: Restyle `TableBlock`** (renderers.tsx:268-286) to use the new `.cp-*` classes and category-colored cells where a cell's column looks like a category (best-effort — only apply color to cells in a column literally named "Category"):

```tsx
function TableBlock({ title, columns, rows }: Extract<CopilotResponseBlock, { kind: "table" }>) {
  const categoryColIndex = columns.findIndex((c) => c.toLowerCase() === "category");
  return (
    <div className="cp-card">
      {title && <div className="cp-card-title" style={{ marginBottom: 12 }}>{title}</div>}
      <table className="tbl">
        <thead>
          <tr>{columns.map((column) => <th key={column}>{column}</th>)}</tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr key={index}>
              {row.map((cell, cellIndex) => (
                <td key={cellIndex}>
                  {cellIndex === categoryColIndex ? (
                    <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                      <span className="cp-dot" style={{ background: colorForCategoryLabel(cell) ?? "var(--ink-faint)" }} />
                      {cell}
                    </span>
                  ) : cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
```

- [ ] **Step 4: Restyle `MetricGrid` and `CalloutBlock`** (renderers.tsx:254-266, 309-316) to wrap in `.cp-card` for visual consistency with the new cards (keep the existing inner `copilot-gen-*` class names for the grid/callout internals since their CSS already exists and matches the tone-based coloring):

```tsx
function MetricGrid({ metrics }: Extract<CopilotResponseBlock, { kind: "metricGrid" }>) {
  return (
    <div className="cp-card">
      <div className="copilot-gen-grid">
        {metrics.map((metric) => (
          <div key={`${metric.label}-${metric.value}`} className="copilot-gen-metric" data-tone={metric.tone ?? "neutral"}>
            <span>{metric.label}</span>
            <strong>{metric.value}</strong>
            {metric.detail && <small>{metric.detail}</small>}
          </div>
        ))}
      </div>
    </div>
  );
}

function CalloutBlock({ tone, title, body }: Extract<CopilotResponseBlock, { kind: "callout" }>) {
  return (
    <div className="cp-card copilot-gen-callout" data-tone={tone}>
      {title && <strong>{title}</strong>}
      <p>{body}</p>
    </div>
  );
}
```

- [ ] **Step 5: Run the renderer test suite again**

Run: `cd ui && npx vitest run src/components/copilot/renderers.test.tsx`
Expected: PASS (structure changed, but the asserted text content is unchanged).

- [ ] **Step 6: Full regression**

Run: `cd ui && npx vitest run && npx tsc --noEmit`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add ui/src/components/copilot/renderers.tsx ui/src/components/copilot/renderers.test.tsx
git commit -m "style(copilot): restyle table/metricGrid/callout blocks into the cp-card frame"
```

---

## Phase D: Reasoning-engine Plan step

### Task D1: Add `plan: Vec<String>` and parse it from the first assistant turn

**Files:**
- Modify: `crates/finsight-agent/src/reasoning/messages.rs` (`ReasoningResult`)
- Modify: `crates/finsight-agent/src/reasoning/engine/mod.rs` (`ReasoningEngineEvent`, `run_with_events`, `build_system_prompt`)
- Test: `crates/finsight-agent/src/reasoning/engine/tests.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/finsight-agent/src/reasoning/engine/tests.rs` (check its existing structure first via `grep -n "fn " crates/finsight-agent/src/reasoning/engine/tests.rs | head -20` to match the file's existing mock-provider pattern, then add):

```rust
#[tokio::test]
async fn run_with_events_emits_plan_ready_before_any_tool_call() {
    let mut events = Vec::new();
    let provider = mock_provider_returning(vec![
        // First turn: a plan prefix followed by tool calls, matching the
        // system-prompt contract added in build_system_prompt.
        AssistantTurn::ToolCalls(vec![ToolCall {
            id: "call-1".to_string(),
            name: "get_financial_snapshot".to_string(),
            arguments: serde_json::json!({}),
        }]),
        AssistantTurn::FinalAnswer {
            content: r#"{"answer":"Done.","reasoning":"","assumptions":[],"data_sources":[],"missing_data":[],"follow_up_questions":[],"response_blocks":[]}"#.to_string(),
            reasoning: String::new(),
        },
    ]);
    // ... use the test file's existing fresh_db()/fresh_conn() helper here ...
    let (_d, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let tools = ToolSet::new(vec![]); // no real tools needed; ToolCallStart still fires
    let _ = ReasoningEngine::run_with_events(&mut conn, "What's my net worth?", &tools, provider, 5, |event| {
        events.push(event);
    })
    .await;

    let plan_index = events.iter().position(|e| matches!(e, ReasoningEngineEvent::PlanReady { .. }));
    let tool_start_index = events.iter().position(|e| matches!(e, ReasoningEngineEvent::ToolCallStart { .. }));
    assert!(plan_index.is_some(), "expected a PlanReady event");
    assert!(tool_start_index.is_some(), "expected a ToolCallStart event");
    assert!(plan_index.unwrap() < tool_start_index.unwrap(), "plan must be emitted before the first tool call");
}
```

(If `mock_provider_returning`/`fresh_db` helpers don't exist under those exact names, `grep -n "fn mock_provider\|fn fresh_db\|struct.*MockProvider" crates/finsight-agent/src/reasoning/engine/tests.rs` first and use the file's actual existing helper names — this file already has a test-provider mocking pattern since `run_with_events` is exercised by other tests in it.)

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p finsight-agent run_with_events_emits_plan_ready --lib`
Expected: FAIL — `ReasoningEngineEvent::PlanReady` does not exist.

- [ ] **Step 3: Add the `plan` field and event variant**

In `crates/finsight-agent/src/reasoning/messages.rs`, add `pub plan: Vec<String>,` to `ReasoningResult` (after `pub reasoning: String,`, before `pub trace: Vec<String>,`).

In `crates/finsight-agent/src/reasoning/engine/mod.rs`, add the variant to `ReasoningEngineEvent` (lines 13-24):

```rust
pub enum ReasoningEngineEvent {
    PlanReady {
        steps: Vec<String>,
    },
    ToolCallStart {
        call: ToolCall,
    },
    ToolCallResult {
        tool_call_id: String,
        tool_name: String,
        result: Value,
        is_error: bool,
    },
}
```

- [ ] **Step 4: Parse the plan from the first turn and emit it before dispatching that turn's tool calls**

Modify `run_with_events` (lines 37-121). The loop needs to track whether it's on the first iteration and extract a plan preamble from that turn's content. Since `AssistantTurn::ToolCalls(Vec<ToolCall>)` doesn't carry raw text content today, add a plan-extraction step that runs BEFORE the tool-call loop, driven by a system-prompt contract: the model's first response, even when it also requests tool calls, must include a fenced `<plan>` block that the provider surfaces via a new field. Since `AssistantTurn` doesn't currently carry this, extend it:

In `crates/finsight-agent/src/reasoning/messages.rs`, change `AssistantTurn`:

```rust
#[derive(Debug, Clone)]
pub enum AssistantTurn {
    ToolCalls { calls: Vec<ToolCall>, plan: Option<Vec<String>> },
    FinalAnswer { content: String, reasoning: String },
}
```

This changes an existing variant's shape, so update every match on `AssistantTurn::ToolCalls(...)`:

Run: `grep -rn "AssistantTurn::ToolCalls" crates/` to find every call site (expected: `engine/mod.rs`'s match arm, and the provider implementations in `finsight-providers` that construct it — check `crates/finsight-providers/src/` for `ollama`/`openai`-style completion providers implementing `complete_tool_turn`).

In `engine/mod.rs`, update the match arm (line 62-96):

```rust
                AssistantTurn::ToolCalls { calls, plan } => {
                    if is_first_turn {
                        if let Some(steps) = plan.filter(|s| !s.is_empty()) {
                            on_event(ReasoningEngineEvent::PlanReady { steps: steps.clone() });
                            result_plan = steps;
                        }
                        is_first_turn = false;
                    }
                    let mut tool_result_msgs = Vec::new();
                    for call in &calls {
```

(add `let mut is_first_turn = true;` and `let mut result_plan: Vec<String> = Vec::new();` near the other `let mut` declarations at the top of `run_with_events`, and thread `result_plan` into every `ReasoningResult { .. }` construction in this file — `parse_final_answer`'s two return sites and the max-iterations fallback — via a new `plan` parameter.)

For each provider in `crates/finsight-providers/src/` implementing `complete_tool_turn` (locate via `grep -rln "fn complete_tool_turn" crates/finsight-providers/src/`), when the model's response includes tool calls, parse a plan preamble from the raw content before the tool-call JSON using a simple line-based extraction (look for a `PLAN:` prefix followed by numbered lines, terminated by a blank line or the tool-call section) and pass it as `AssistantTurn::ToolCalls { calls, plan: extracted_plan }`; when no plan preamble is present (e.g. a later turn in the same run), pass `plan: None`.

- [ ] **Step 5: Update `build_system_prompt`** (`engine/mod.rs:167-220`) to require the plan preamble on the first turn — insert after the `GROUNDING RULE` sentence (line 185):

```
PLANNING: before your first tool call (or before your first answer, if no tools are needed), output a short plan as plain lines prefixed `PLAN:` followed by 3-5 numbered one-sentence steps, then a blank line, before anything else. Example:\nPLAN:\n1. Find the income that just landed\n2. Rank every debt by interest rate\n3. Recommend where each dollar should go\n\nDo this only once, on your very first response in this conversation turn — never repeat it on later tool-calling turns within the same question.\n\
```

- [ ] **Step 6: Run the test again**

Run: `cargo test -p finsight-agent run_with_events_emits_plan_ready --lib`
Expected: PASS.

- [ ] **Step 7: Run the full `finsight-agent` and `finsight-providers` test suites**

Run: `cargo test -p finsight-agent --lib && cargo test -p finsight-providers --lib`
Expected: all pass — fix any test constructing `AssistantTurn::ToolCalls(...)` with the old tuple shape to use the new `{ calls, plan }` struct-variant shape instead (the compiler will point at every one via exhaustive match/construction errors).

- [ ] **Step 8: Run the full Rust workspace**

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add crates/finsight-agent/src/reasoning/messages.rs crates/finsight-agent/src/reasoning/engine/mod.rs crates/finsight-agent/src/reasoning/engine/tests.rs crates/finsight-providers/src/
git commit -m "feat(agent): parse an upfront Plan step from the first assistant turn"
```

### Task D2: Thread the Plan through the AG-UI stream frame and persist it

**Files:**
- Modify: `crates/finsight-app/src/commands/copilot_chat.rs` (`CopilotStreamFrame`, event handling in `stream_copilot_message`)
- Modify: `ui/src/api/client.ts` (`CopilotStreamFrame` type)
- Modify: `ui/src/components/copilot/agUi/TauriAgUiAgent.ts` (handle the new frame type)
- Modify: `ui/src/components/copilot/TauriRuntime.ts` (`MessageMeta.plan`, persistence read-back)
- Test: `ui/src/components/copilot/agUi/TauriAgUiAgent.test.ts`

- [ ] **Step 1: Add the Rust stream frame variant**

In `crates/finsight-app/src/commands/copilot_chat.rs`, find the `CopilotStreamFrame` enum (near the `ResponseBlock`/`Source` variants seen earlier around line 78) and add:

```rust
    Plan {
        conversation_id: String,
        run_id: String,
        thread_id: String,
        assistant_message_id: String,
        parent_message_id: Option<String>,
        sequence_number: u64,
        steps: Vec<String>,
    },
```

- [ ] **Step 2: Emit it from `stream_copilot_message`'s event handler**

In the `move |event| match event { ... }` closure (copilot_chat.rs:344-384), add a new match arm before `ReasoningEngineEvent::ToolCallStart`:

```rust
                        ReasoningEngineEvent::PlanReady { steps } => {
                            emit_copilot_frame(
                                &app_for_events,
                                CopilotStreamFrame::Plan {
                                    conversation_id: event_conversation_id.clone(),
                                    run_id: event_run_id.clone(),
                                    thread_id: event_conversation_id.clone(),
                                    assistant_message_id: event_assistant_message_id.clone(),
                                    parent_message_id: event_parent_message_id.clone(),
                                    sequence_number: event_sequence.fetch_add(1, Ordering::Relaxed),
                                    steps,
                                },
                            );
                        }
```

- [ ] **Step 3: Persist it in `agUiMetadataJson`**

Find where `agUiMetadataJson` is built for persistence after the run completes (search `grep -n "agUiMetadataJson\|toolTrace" crates/finsight-app/src/commands/copilot_chat.rs` — likely near where `answer.trace`/`follow_up_questions` get serialized into the metadata JSON object saved via `conversations::insert_message`/`update_user_message`-equivalent for the assistant message). Add `"plan": answer.plan` (requires threading `plan: Vec<String>` onto `AgentAnswer` too — add `pub plan: Vec<String>,` to the `AgentAnswer` struct in `agent.rs:527-541`, and populate it from `ReasoningResult.plan` in `reasoning_result_to_agent_answer` alongside the existing `trace`/`follow_up_questions` mapping).

- [ ] **Step 4: Run the Rust workspace**

Run: `cargo test --workspace`
Expected: pass — fix any `AgentAnswer { .. }` construction site missing the new `plan` field (the compiler enumerates them).

- [ ] **Step 5: Regenerate bindings**

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: `CopilotStreamFrame` in `bindings.ts` gains the `{ type: "plan"; steps: string[] } & CopilotStreamFrameMeta` member, and `AgentAnswer` gains `plan: string[]`.

- [ ] **Step 6: Write the failing frontend test**

```ts
// add to ui/src/components/copilot/agUi/TauriAgUiAgent.test.ts (check existing structure first via Read)
it("maps a plan stream frame into a PLAN custom event carrying the steps", () => {
  // follow this file's existing pattern for constructing a frame and calling
  // the same mapping function exercised by its other "case" tests (search this
  // file for how it tests the "responseBlock"/"toolCallResult" cases and mirror
  // that setup exactly).
});
```

(Read `ui/src/components/copilot/agUi/TauriAgUiAgent.test.ts` in full first — it already tests every other frame-to-event mapping case; add the `plan` case test following its exact existing style, e.g. constructing a raw `{ type: "plan", ... }` frame object and asserting on the emitted event(s).)

- [ ] **Step 7: Run it to verify it fails**

Run: `cd ui && npx vitest run src/components/copilot/agUi/TauriAgUiAgent.test.ts`
Expected: FAIL.

- [ ] **Step 8: Handle the frame in `TauriAgUiAgent.ts`**

Find the `FRAME_TYPE_ALIASES`-style mapping near line 64-65 (`responseBlock: "responseBlock", response_block: "responseBlock",`) and add:

```ts
    plan: "plan",
```

Find the `case "responseBlock":` handling (around line 121 and 293) and add a sibling case:

```ts
    case "plan": {
      const planFrame = frame as Extract<CopilotStreamFrame, { type: "plan" }>;
      emit({
        type: "CUSTOM",
        name: "finsight.plan",
        value: { steps: planFrame.steps },
      } as BaseEvent);
      break;
    }
```

- [ ] **Step 9: Run the test again**

Run: `cd ui && npx vitest run src/components/copilot/agUi/TauriAgUiAgent.test.ts`
Expected: PASS.

- [ ] **Step 10: Surface `plan` on `MessageMeta` and read it back from persistence**

In `ui/src/components/copilot/TauriRuntime.ts`, add `plan?: string[];` to `MessageMeta` (line 33-43), and in the `agUiMetadataJson` parsing block (lines 572-585) add:

```ts
          if (Array.isArray(parsed.plan)) {
            meta.plan = parsed.plan.filter((item): item is string => typeof item === "string");
          }
```

For the live (non-reload) path, find where `TauriAgUiRuntime.ts` accumulates per-message metadata from CUSTOM events (search `grep -n "finsight\." ui/src/components/copilot/agUi/TauriAgUiRuntime.ts` to find the existing pattern for other `finsight.*` custom events like follow-ups) and add the same accumulation for `finsight.plan` → `meta.plan`.

- [ ] **Step 11: Wire `meta.plan` into the `ThinkingBlock`**

Back in `ui/src/screens/Copilot.tsx`, update `ThinkingBlock` (from Task A4) to accept and render an optional plan:

```tsx
function ThinkingBlock({ reasoningText, toolCalls, plan }: { reasoningText: string; toolCalls: ReactNode; plan?: string[] }) {
  // ...existing state/logic unchanged...
  return (
    <div className={`cp-think ${isRunning ? "is-running" : "is-done"}`}>
      {/* ...existing header unchanged... */}
      {open && (
        <div className="cp-think-body">
          {plan && plan.length > 0 && (
            <div className="cp-think-sec">
              <p className="cp-think-sec-lbl">Plan</p>
              <div className="cp-think-plan">
                {plan.map((step, i) => (
                  <div key={i} className="cp-plan-item">
                    <span className="cp-plan-n">{i + 1}</span>
                    <span className="cp-plan-txt">{step}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
          <div className="cp-think-sec">
            <p className="cp-think-sec-lbl">Tool calls</p>
            <div className="cp-think-tools">{toolCalls}</div>
          </div>
          {/* ...existing Reasoning section unchanged... */}
        </div>
      )}
    </div>
  );
}
```

Update the call site in `AssistantMessage`'s `"group-thought"` case (Task A4) to pass `plan={meta?.plan}` (on the legacy `TauriRuntime.ts` path, `meta.plan` is simply always `undefined`, so the Plan section correctly never renders there — satisfying the spec's runtime-scope decision with zero extra branching).

- [ ] **Step 12: Full regression**

Run: `cargo test --workspace && cd ui && npx vitest run && npx tsc --noEmit`
Expected: all green.

- [ ] **Step 13: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs ui/src/api/bindings.ts ui/src/api/client.ts ui/src/components/copilot/agUi/TauriAgUiAgent.ts ui/src/components/copilot/agUi/TauriAgUiAgent.test.ts ui/src/components/copilot/agUi/TauriAgUiRuntime.ts ui/src/components/copilot/TauriRuntime.ts ui/src/screens/Copilot.tsx
git commit -m "feat(copilot): thread the Plan step through AG-UI streaming and persistence"
```

---

## Phase E: RecategorizationPreview synthesis + real CSV export

### Task E1: `RecategorizationPreview` artifact kind (backend struct + gates only — synthesis wired in E2)

**Files:** same three-gate files as Phase C.

- [ ] **Step 1: Rust test** —

```rust
#[test]
fn recategorization_preview_round_trips_and_validates() {
    let block = AgentResponseBlock::RecategorizationPreview(AgentRecategorizationPreviewBlock {
        count: 23,
        rows: vec![AgentRecatRow { merchant: "Trader Joe's".to_string(), category_key: "Groceries".to_string(), confidence: 0.99 }],
        more: 18,
        bundle_id: "bundle-abc".to_string(),
    });
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["kind"], "recategorizationPreview");
    let back: AgentResponseBlock = serde_json::from_value(json).unwrap();
    assert!(valid_response_block(&back));
}

#[test]
fn recategorization_preview_with_empty_bundle_id_is_invalid() {
    let block = AgentResponseBlock::RecategorizationPreview(AgentRecategorizationPreviewBlock {
        count: 1,
        rows: vec![AgentRecatRow { merchant: "X".to_string(), category_key: "Y".to_string(), confidence: 0.9 }],
        more: 0,
        bundle_id: "".to_string(),
    });
    assert!(!valid_response_block(&block));
}
```

- [ ] **Step 2: Run, verify failure.**

- [ ] **Step 3: Implement in `agent.rs`** —

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentRecatRow {
    pub merchant: String,
    pub category_key: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentRecategorizationPreviewBlock {
    pub count: i64,
    pub rows: Vec<AgentRecatRow>,
    pub more: i64,
    pub bundle_id: String,
}
```

Enum variant: `RecategorizationPreview(AgentRecategorizationPreviewBlock),`. Validation:

```rust
        AgentResponseBlock::RecategorizationPreview(b) => {
            !b.bundle_id.trim().is_empty() && !b.rows.is_empty() && b.rows.len() <= 20
        }
```

- [ ] **Step 4: Run Rust test again** — PASS.

- [ ] **Step 5: `copilot_chat.rs` gates** — `should_emit_response_block`: `AgentResponseBlock::RecategorizationPreview(_) => true,`. Bounds:

```rust
        AgentResponseBlock::RecategorizationPreview(b) => {
            b.rows.len() <= 20
                && b.rows.iter().all(|r| label_ok(&r.merchant) && label_ok(&r.category_key))
                && label_ok(&b.bundle_id)
        }
```

- [ ] **Step 6: Full Rust suite, regenerate bindings.**

- [ ] **Step 7: Zod schema** —

```ts
  z.object({
    kind: z.literal("recategorizationPreview"),
    count: z.number().int().nonnegative(),
    rows: z.array(z.object({ merchant: shortString, categoryKey: shortString, confidence: z.number().min(0).max(1) })).min(1).max(20),
    more: z.number().int().nonnegative(),
    bundleId: z.string().min(1).max(MAX_LABEL),
  }),
```

- [ ] **Step 8: Frontend test** —

```tsx
// ui/src/components/copilot/cards/RecategorizationPreviewCard.test.tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { createWrapper } from "../../../test-utils";
import { RecategorizationPreviewCard } from "./RecategorizationPreviewCard";

vi.mock("../../../api/hooks/copilot", () => ({
  useActionBundle: vi.fn(() => ({ data: { id: "bundle-abc", items: [] }, isLoading: false })),
  useApproveActionItem: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useRejectActionItem: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useExecuteActionBundle: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("RecategorizationPreviewCard", () => {
  it("renders each proposed merchant/category/confidence row and a more-count footer", () => {
    render(
      <RecategorizationPreviewCard
        block={{
          kind: "recategorizationPreview",
          count: 23,
          rows: [{ merchant: "Trader Joe's", categoryKey: "Groceries", confidence: 0.99 }],
          more: 18,
          bundleId: "bundle-abc",
        }}
      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("Trader Joe's")).toBeInTheDocument();
    expect(screen.getByText("Groceries")).toBeInTheDocument();
    expect(screen.getByText("99%")).toBeInTheDocument();
    expect(screen.getByText(/\+ 18 more/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 9: Run, verify failure.**

- [ ] **Step 10: Implement, reusing the existing `ActionApprovalToolCard`'s bundle-fetching pattern from `renderers.tsx` rather than duplicating it** —

```tsx
// ui/src/components/copilot/cards/RecategorizationPreviewCard.tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import { ActionApprovalToolCard } from "../renderers";
import { ConfidenceBadge } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "recategorizationPreview" }>;

export function RecategorizationPreviewCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <div className="cp-card-head">
        <div className="cp-card-title">{block.count} categorizations proposed</div>
        <div className="cp-card-sub">all proposed changes await your approval below</div>
      </div>
      <div className="cp-recat">
        {block.rows.map((r, i) => {
          const color = colorForCategoryLabel(r.categoryKey) ?? "var(--ink-faint)";
          return (
            <div key={i} className="cp-recat-row">
              <span className="cp-recat-merchant">{r.merchant}</span>
              <span className="cp-recat-cat" style={{ color, borderColor: color }}>
                <span className="cp-dot" style={{ background: color }} />
                {r.categoryKey}
              </span>
              <ConfidenceBadge confidence={r.confidence} color={color} />
            </div>
          );
        })}
        {block.more > 0 && <div className="cp-tx-more">+ {block.more} more matched the same way</div>}
      </div>
      {/* Never a standalone mutation — approve/reject/execute is the existing real flow. */}
      <div style={{ marginTop: 14 }}>
        <ActionApprovalToolCard bundleId={block.bundleId} />
      </div>
    </div>
  );
}
```

`ActionApprovalToolCard` (currently a private function in `renderers.tsx:140-252`) needs to be exported for this import to work — change `function ActionApprovalToolCard` to `export function ActionApprovalToolCard` in `renderers.tsx`.

- [ ] **Step 11: Register in `renderers.tsx`** — `case "recategorizationPreview": return <RecategorizationPreviewCard block={block} />;`

- [ ] **Step 12: Run frontend test again** — PASS.

- [ ] **Step 13: CSS** —

```css
.cp-recat { display: flex; flex-direction: column; gap: 2px; }
.cp-recat-row { display: grid; grid-template-columns: 1fr auto 110px; gap: 11px; align-items: center; padding: 9px 0; border-bottom: 1px solid var(--hairline); }
.cp-recat-merchant { font-size: 13.5px; color: var(--ink); }
.cp-recat-cat { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; font-weight: 500; padding: 3px 9px; border-radius: 999px; border: 1px solid; background: var(--surface-2); }
.cp-conf { display: flex; align-items: center; gap: 8px; }
.cp-conf-track { flex: 1; height: 5px; background: var(--bg-2); border-radius: 999px; overflow: hidden; }
.cp-conf-fill { height: 100%; border-radius: 999px; }
.cp-conf-num { font-family: var(--mono); font-size: 11px; color: var(--ink-mute); width: 30px; text-align: right; }
```

- [ ] **Step 14: Full regression, commit.**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs ui/src/api/bindings.ts ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/cards/RecategorizationPreviewCard.tsx ui/src/components/copilot/cards/RecategorizationPreviewCard.test.tsx ui/src/components/copilot/renderers.tsx ui/src/styles/copilot-shell.css
git commit -m "feat(copilot): add RecategorizationPreview artifact kind (card only, synthesis next)"
```

### Task E2: Synthesize `RecategorizationPreview` server-side (never model-chosen)

**Files:**
- Modify: `crates/finsight-app/src/commands/copilot_chat.rs` (after bundle persistence, ~line 452)
- Test: `crates/finsight-app/src/commands/copilot_chat.rs` (inline test)

- [ ] **Step 1: Write the failing test**

Find the existing test module in `copilot_chat.rs` (search `grep -n "mod tests" crates/finsight-app/src/commands/copilot_chat.rs`) and add:

```rust
#[test]
fn synthesize_recategorization_preview_builds_a_block_from_recategorize_bulk_draft_actions() {
    let draft_actions = vec![finsight_agent::reasoning::messages::AgentDraftAction {
        action_kind: "recategorize_bulk".to_string(),
        payload_json: serde_json::json!({
            "assignments": [
                { "transactionId": "t1", "categoryId": "c1", "categoryLabel": "Groceries", "merchant": "Trader Joe's", "confidence": 0.99 },
                { "transactionId": "t2", "categoryId": "c2", "categoryLabel": "Transport", "merchant": "Shell", "confidence": 0.97 },
            ]
        }).to_string(),
        rationale: "Recategorize 2 uncategorized transactions.".to_string(),
        confidence: 0.98,
    }];

    let block = synthesize_recategorization_preview(&draft_actions, "bundle-xyz");
    assert!(block.is_some());
    let AgentResponseBlock::RecategorizationPreview(preview) = block.unwrap() else {
        panic!("expected a RecategorizationPreview block");
    };
    assert_eq!(preview.bundle_id, "bundle-xyz");
    assert_eq!(preview.count, 2);
    assert_eq!(preview.rows.len(), 2);
    assert_eq!(preview.rows[0].merchant, "Trader Joe's");
    assert_eq!(preview.more, 0);
}

#[test]
fn synthesize_recategorization_preview_returns_none_without_a_recategorize_bulk_action() {
    let draft_actions = vec![finsight_agent::reasoning::messages::AgentDraftAction {
        action_kind: "set_budget".to_string(),
        payload_json: "{}".to_string(),
        rationale: "unrelated".to_string(),
        confidence: 0.9,
    }];
    assert!(synthesize_recategorization_preview(&draft_actions, "bundle-xyz").is_none());
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p finsight-app synthesize_recategorization_preview --lib`
Expected: FAIL — function does not exist.

- [ ] **Step 3: Implement the synthesis function**

Add to `crates/finsight-app/src/commands/copilot_chat.rs` (near `response_block_part`, around line 1078):

```rust
/// Synthesizes a `RecategorizationPreview` block from a turn's draft actions.
/// This is the ONE artifact kind the model never chooses via response_blocks —
/// its bundle_id only exists after the action bundle is persisted (see
/// insert_bundle/insert_item above), which happens after the reasoning loop
/// returns. Reads the same preview data draft_recategorization already
/// computed (act.rs) straight out of the draft action's payload_json.
fn synthesize_recategorization_preview(
    draft_actions: &[finsight_agent::reasoning::messages::AgentDraftAction],
    bundle_id: &str,
) -> Option<AgentResponseBlock> {
    let draft = draft_actions.iter().find(|d| d.action_kind == "recategorize_bulk")?;
    let payload: serde_json::Value = serde_json::from_str(&draft.payload_json).ok()?;
    let assignments = payload.get("assignments")?.as_array()?;
    if assignments.is_empty() {
        return None;
    }

    const PREVIEW_ROWS: usize = 5;
    let rows: Vec<AgentRecatRow> = assignments
        .iter()
        .take(PREVIEW_ROWS)
        .filter_map(|a| {
            Some(AgentRecatRow {
                merchant: a.get("merchant")?.as_str()?.to_string(),
                category_key: a.get("categoryLabel")?.as_str()?.to_string(),
                confidence: a.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.7),
            })
        })
        .collect();
    if rows.is_empty() {
        return None;
    }

    Some(AgentResponseBlock::RecategorizationPreview(AgentRecategorizationPreviewBlock {
        count: assignments.len() as i64,
        more: (assignments.len().saturating_sub(rows.len())) as i64,
        rows,
        bundle_id: bundle_id.to_string(),
    }))
}
```

- [ ] **Step 4: Run the test again**

Run: `cargo test -p finsight-app synthesize_recategorization_preview --lib`
Expected: PASS.

- [ ] **Step 5: Call it right after the bundle is persisted**

In `stream_copilot_message`, right after `let mut answer = reasoning_result_to_agent_answer(result, bundle_id);` (copilot_chat.rs:452), before `validate_finance_answer(...)`:

```rust
            if let Some(bid) = &bundle_id {
                if let Some(preview_block) = synthesize_recategorization_preview(&draft_actions, bid) {
                    answer.response_blocks.push(preview_block);
                }
            }
```

(`draft_actions` is already in scope at this point from line 405 — `let draft_actions = result.draft_actions.clone();`.)

- [ ] **Step 6: Run the full Rust workspace**

Run: `cargo test --workspace`
Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/finsight-app/src/commands/copilot_chat.rs
git commit -m "feat(copilot): synthesize RecategorizationPreview from recategorize_bulk draft actions"
```

### Task E3: Real CSV export for TransactionTable

**Files:**
- Modify: `crates/finsight-agent/src/reasoning/tools/read.rs` (extract shared filter-building)
- Modify: `crates/finsight-app/src/commands/transactions.rs` (new command)
- Modify: `crates/finsight-app/src/lib.rs` (register command)
- Modify: `ui/src/components/copilot/cards/TransactionTableCard.tsx` (Export button)
- Test: `crates/finsight-core/tests/` (new integration test) + `ui/src/components/copilot/cards/TransactionTableCard.test.tsx` (extend)

- [ ] **Step 1: Extract the shared query struct and SQL builder**

In `crates/finsight-agent/src/reasoning/tools/read.rs`, the `search_transactions` tool (lines 368-451) builds SQL inline inside `execute()`. Extract the filter-building into a public, reusable function in `finsight-core` so both the tool and the new export command call the same code. Add to `crates/finsight-core/src/repos/transactions.rs` (check its existing `list`/`TxnFilter` location first via `grep -n "pub struct TxnFilter\|pub fn list" crates/finsight-core/src/repos/transactions.rs`):

```rust
#[derive(Debug, Clone, Default)]
pub struct SearchTxnQuery {
    pub merchant: Option<String>,
    pub account: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub min_amount_cents: Option<i64>,
    pub direction: Option<String>, // "expense" | "income" | None
}

pub struct SearchTxnRow {
    pub date: String,
    pub merchant: String,
    pub amount_cents: i64,
    pub account: String,
    pub category: String,
}

/// Shared query builder for both the `search_transactions` Copilot tool and
/// the Copilot "Export as CSV" command — one canonical filter implementation
/// instead of two SQL strings that could drift apart.
pub fn search(conn: &Connection, query: &SearchTxnQuery, limit: i64) -> CoreResult<Vec<SearchTxnRow>> {
    let mut sql = "SELECT t.merchant_raw, t.amount_cents, t.posted_at, COALESCE(c.label, 'Uncategorized'), COALESCE(a.name, 'Unknown account') \
         FROM transactions t \
         LEFT JOIN categories c ON c.id = t.category_id \
         LEFT JOIN accounts a ON a.id = t.account_id \
         WHERE 1=1".to_string();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(m) = &query.merchant {
        sql.push_str(" AND lower(t.merchant_raw) LIKE lower(?)");
        params.push(Box::new(format!("%{}%", m)));
    }
    if let Some(acct) = &query.account {
        sql.push_str(" AND lower(a.name) LIKE lower(?)");
        params.push(Box::new(format!("%{}%", acct)));
    }
    if let Some(s) = &query.start_date {
        sql.push_str(" AND t.posted_at >= ?");
        params.push(Box::new(s.clone()));
    }
    if let Some(e) = &query.end_date {
        sql.push_str(" AND t.posted_at <= ?");
        params.push(Box::new(format!("{}T23:59:59", e)));
    }
    if let Some(min) = query.min_amount_cents {
        sql.push_str(" AND ABS(t.amount_cents) >= ?");
        params.push(Box::new(min.abs()));
    }
    match query.direction.as_deref() {
        Some("expense") => sql.push_str(" AND t.amount_cents < 0"),
        Some("income") => sql.push_str(" AND t.amount_cents > 0"),
        _ => {}
    }
    sql.push_str(" ORDER BY t.posted_at DESC LIMIT ?");
    params.push(Box::new(limit));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())), |r| {
        Ok(SearchTxnRow {
            merchant: r.get(0)?,
            amount_cents: r.get(1)?,
            date: r.get(2)?,
            category: r.get(3)?,
            account: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
```

- [ ] **Step 2: Rewrite `search_transactions`'s tool `execute()` to call the shared function**

In `crates/finsight-agent/src/reasoning/tools/read.rs` (lines 388-448), replace the inline SQL-building with:

```rust
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let query = finsight_core::repos::transactions::SearchTxnQuery {
                merchant: args["merchant"].as_str().map(String::from),
                account: args["account"].as_str().map(String::from),
                start_date: args["start_date"].as_str().map(String::from),
                end_date: args["end_date"].as_str().map(String::from),
                min_amount_cents: args["min_amount_cents"].as_i64(),
                direction: args["direction"].as_str().filter(|d| *d != "any").map(String::from),
            };
            let limit = args["limit"].as_i64().unwrap_or(50).clamp(1, 500);
            let rows = finsight_core::repos::transactions::search(ctx.conn, &query, limit)?;

            let response_rows: Vec<Value> = rows
                .iter()
                .map(|r| json!({ "date": r.date, "merchant": r.merchant, "amount_cents": r.amount_cents, "account": r.account, "category": r.category }))
                .collect();
            let total_cents: i64 = rows.iter().map(|r| r.amount_cents).sum();
            let total_abs_cents: i64 = rows.iter().map(|r| r.amount_cents.abs()).sum();
            Ok(json!({
                "transactions": response_rows,
                "count": rows.len(),
                "total_cents": total_cents,
                "total_abs_cents": total_abs_cents,
                "capped": rows.len() as i64 == limit
            }))
        }
```

- [ ] **Step 3: Run the existing tool tests** (already cover `search_transactions_filters_by_date_range_and_amount_threshold` and `search_transactions_filters_by_account` per read.rs:958,992)

Run: `cargo test -p finsight-agent search_transactions --lib`
Expected: PASS unchanged — the refactor preserves identical query semantics.

- [ ] **Step 4: Write the failing export command test**

Create `crates/finsight-core/tests/repos_transactions_search.rs`:

```rust
use finsight_core::db::run_migrations;
use finsight_core::keychain;
use finsight_core::models::{AccountType, NewAccount, NewTransaction, TransactionStatus};
use finsight_core::repos::{accounts, transactions};
use finsight_core::Db;
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("t.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

#[test]
fn search_filters_by_account_substring_and_min_amount() {
    let (_d, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let amex = accounts::insert(&mut conn, base_account("Amex Card", 0, "manual")).unwrap();
    let chase = accounts::insert(&mut conn, base_account("Chase Checking", 0, "manual")).unwrap();

    transactions::insert(&mut conn, mk_txn(&amex.id, -7_000, "2026-05-10")).unwrap();
    transactions::insert(&mut conn, mk_txn(&amex.id, -3_000, "2026-05-11")).unwrap();
    transactions::insert(&mut conn, mk_txn(&chase.id, -9_000, "2026-05-12")).unwrap();

    let query = transactions::SearchTxnQuery {
        account: Some("amex".to_string()),
        min_amount_cents: Some(6_000),
        ..Default::default()
    };
    let rows = transactions::search(&conn, &query, 50).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].amount_cents, -7_000);
}

fn mk_txn(account_id: &str, amount_cents: i64, date: &str) -> NewTransaction {
    NewTransaction {
        account_id: account_id.to_string(),
        posted_at: chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap().and_hms_opt(12, 0, 0).unwrap().and_utc(),
        amount_cents,
        merchant_raw: "M".to_string(),
        category_id: None,
        notes: None,
        status: TransactionStatus::Cleared,
        imported_id: None,
        source: None,
        raw_synced_data: None,
        pending: false,
        external_tx_id: None,
        external_account_id: None,
    }
}

fn base_account(name: &str, opening_balance_cents: i64, source: &str) -> NewAccount {
    NewAccount {
        owner: "me".to_string(), bank: "Bank".to_string(), r#type: AccountType::Checking, name: name.to_string(),
        last4: None, currency: "USD".to_string(), color: "#3B82F6".to_string(), source: source.to_string(),
        liquidity_type: "liquid".to_string(), emergency_fund_eligible: true, goal_earmark: None, apy_pct: None,
        opening_balance_cents, simplefin_account_id: None, nickname: None, connection_id: None, institution_id: None,
        external_account_id: None, official_name: None, mask: None, subtype: None, account_group: "cash".to_string(),
        available_balance_cents: None, balance_date: None, extra_json: None, raw_json: None, import_pending: false,
        apr_pct: None, min_payment_cents: None, payoff_date: None, limit_cents: None, original_balance_cents: None, started_at: None,
    }
}
```

- [ ] **Step 5: Run it to verify it fails**

Run: `cargo test -p finsight-core --test repos_transactions_search`
Expected: FAIL — `SearchTxnQuery`/`search` don't exist yet (this test is written against Step 1's target API — if Step 1 hasn't landed yet in your working copy, do Step 1 first, then this test should pass immediately; if you're following strict TDD, write this test file before Step 1's implementation instead, confirm the FAIL, then implement).

- [ ] **Step 6: Run it again after Step 1's implementation** — PASS.

- [ ] **Step 7: Add the export command**

In `crates/finsight-app/src/commands/transactions.rs`, after `export_transactions_csv` (line 594), add:

```rust
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct SearchTxnQueryInput {
    pub merchant: Option<String>,
    pub account: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub min_amount_cents: Option<i64>,
    pub direction: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn export_search_transactions_csv(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    query: SearchTxnQueryInput,
) -> AppResult<String> {
    let maybe_path = app
        .dialog()
        .file()
        .set_file_name("transactions.csv")
        .blocking_save_file();

    let Some(file_path) = maybe_path else {
        return Ok(String::new());
    };
    let path = file_path
        .into_path()
        .map_err(|e| AppError::new("dialog", e.to_string()))?;

    let db = (*state.db).clone();
    let csv = run(&db, move |conn| {
        let rows = finsight_core::repos::transactions::search(
            conn,
            &finsight_core::repos::transactions::SearchTxnQuery {
                merchant: query.merchant,
                account: query.account,
                start_date: query.start_date,
                end_date: query.end_date,
                min_amount_cents: query.min_amount_cents,
                direction: query.direction,
            },
            i64::MAX,
        )?;
        let mut out = String::from("date,merchant,category,amount_dollars,account\n");
        for r in rows {
            let date = &r.date[..10.min(r.date.len())];
            let merchant = csv_escape(&r.merchant);
            let category = csv_escape(&r.category);
            let amount = format!("{:.2}", r.amount_cents as f64 / 100.0);
            let account = csv_escape(&r.account);
            out.push_str(&format!("{date},{merchant},{category},{amount},{account}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)?;

    let path_str = path.to_string_lossy().to_string();
    std::fs::write(&path, csv).map_err(|e| AppError::new("io", e.to_string()))?;
    Ok(path_str)
}
```

- [ ] **Step 8: Register the command**

In `crates/finsight-app/src/lib.rs`, find `commands::transactions::export_transactions_csv,` (line 358) and add `commands::transactions::export_search_transactions_csv,` immediately after it.

- [ ] **Step 9: Run the full Rust workspace, regenerate bindings**

Run: `cargo test --workspace && cargo run -p finsight-tauri --bin export_bindings`
Expected: pass; `bindings.ts` gains `exportSearchTransactionsCsv`.

- [ ] **Step 10: Wire the Export button in `TransactionTableCard`**

The card needs the original tool-call args (merchant/account/date range/min_amount/direction) to re-run the export — per the spec, these are NOT duplicated into the block payload; they come from the tool-call part. Thread them down from `CopilotToolCard` (renderers.tsx:72-87), where `args` is already available:

```tsx
    if (block) {
      return <FinSightResponseBlock block={block} isRunning={status.type === "running"} toolArgs={args} />;
    }
```

Update `FinSightResponseBlock` to accept and forward `toolArgs`:

```tsx
export function FinSightResponseBlock({ block, isRunning, toolArgs }: { block: CopilotResponseBlock; isRunning: boolean; toolArgs?: Record<string, unknown> }) {
  switch (block.kind) {
    // ...
    case "transactionTable":
      return <TransactionTableCard block={block} toolArgs={toolArgs} />;
```

Update `TransactionTableCard`:

```tsx
// ui/src/components/copilot/cards/TransactionTableCard.tsx
import { useState } from "react";
import { toast } from "sonner";
import type { CopilotResponseBlock } from "../../../api/client";
import { commands } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import Button from "../../Button";

type Block = Extract<CopilotResponseBlock, { kind: "transactionTable" }>;

export function TransactionTableCard({ block, toolArgs }: { block: Block; toolArgs?: Record<string, unknown> }) {
  const [exporting, setExporting] = useState(false);

  const handleExport = async () => {
    setExporting(true);
    try {
      const result = await commands.exportSearchTransactionsCsv({
        merchant: (toolArgs?.merchant as string) ?? null,
        account: (toolArgs?.account as string) ?? null,
        startDate: (toolArgs?.start_date as string) ?? null,
        endDate: (toolArgs?.end_date as string) ?? null,
        minAmountCents: (toolArgs?.min_amount_cents as number) ?? null,
        direction: (toolArgs?.direction as string) ?? null,
      });
      if (result.status === "ok" && result.data) {
        toast.success("Exported", { description: result.data });
      }
    } catch (err) {
      toast.error("Export failed", { description: String(err) });
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="cp-card">
      <div className="cp-card-head">
        <div className="cp-card-title">{block.count} transactions</div>
        <div className="cp-card-sub">{money(block.totalCents)} total · top {block.rows.length} by size</div>
      </div>
      <div className="cp-tx">
        {block.rows.map((r, i) => (
          <div key={i} className="cp-tx-row">
            <span className="cp-tx-date mono">{r.date}</span>
            <div className="cp-tx-merchant">
              <span className="cp-dot" style={{ background: colorForCategoryLabel(r.categoryKey) ?? "var(--ink-faint)" }} />
              <span>{r.merchant}</span>
              {r.flag && <span className="cp-tx-flag">{r.flag}</span>}
            </div>
            <span className="cp-tx-cat">{r.categoryKey}</span>
            <span className="cp-tx-amt mono">{money(r.amountCents)}</span>
          </div>
        ))}
        {block.more > 0 && (
          <div className="cp-tx-more">+ {block.more} more · {money(block.totalCents)} total</div>
        )}
      </div>
      <div style={{ marginTop: 14 }}>
        <Button variant="primary" size="sm" loading={exporting} disabled={exporting} onClick={() => void handleExport()}>
          Export {block.count} as CSV
        </Button>
      </div>
    </div>
  );
}
```

(The exact `commands.exportSearchTransactionsCsv` camelCase param names above must match whatever `export_bindings` actually generated in Step 9 for `SearchTxnQueryInput` — check `ui/src/api/bindings.ts` after regenerating and adjust field names if tauri-specta's naming differs from this sketch.)

- [ ] **Step 11: Update `TransactionTableCard.test.tsx`** to mock `commands.exportSearchTransactionsCsv` and assert the button renders and calls it on click, following the existing mocking pattern used elsewhere in this codebase for `commands.*` calls (e.g. `AccountTransactions.tsx`'s export test, if one exists — check via `grep -rn "exportTransactionsCsv" ui/src/screens/AccountTransactions.test.tsx`).

- [ ] **Step 12: Full regression**

Run: `cargo test --workspace && cd ui && npx vitest run && npx tsc --noEmit`
Expected: all green.

- [ ] **Step 13: Commit**

```bash
git add crates/finsight-core/src/repos/transactions.rs crates/finsight-core/tests/repos_transactions_search.rs crates/finsight-agent/src/reasoning/tools/read.rs crates/finsight-app/src/commands/transactions.rs crates/finsight-app/src/lib.rs ui/src/api/bindings.ts ui/src/components/copilot/cards/TransactionTableCard.tsx ui/src/components/copilot/cards/TransactionTableCard.test.tsx ui/src/components/copilot/renderers.tsx
git commit -m "feat(copilot): real CSV export for TransactionTable, sharing search_transactions' query logic"
```

---

## Final verification (after all phases land)

- [ ] Run `cargo test --workspace` — expect the pre-existing green bar (341+ tests) plus every new test added across Phases A-E, 0 failures.
- [ ] Run `cd ui && npx vitest run` — expect the pre-existing green bar (294+ tests) plus every new test added, 0 failures.
- [ ] Run `cd ui && npx tsc --noEmit` — expect 0 errors.
- [ ] Start the native app (`pnpm tauri:dev` — NOT the browser preview tool, which cannot drive Tauri IPC per this session's earlier finding) and manually:
  - Ask a question that should trigger `search_transactions` (e.g. "find me all transactions in May in Amex over $60") and confirm the `TransactionTable` card renders with real data and the Export button produces a real CSV.
  - Ask a paycheck-allocation-style question and confirm an `AllocationSplit` or similar card renders.
  - Trigger a recategorization ("clean up my uncategorized transactions") and confirm the `RecategorizationPreview` card shows real proposed rows and its approve/execute buttons work through the existing action-bundle flow.
  - Confirm the thinking block shows a real Plan section before tool calls, sourced from the model's actual first turn.
  - Confirm the hero/hero-empty state, turn header, and follow-up chips visually match the mockup's language while showing 100% real data (no "1,247 transactions" anywhere).
