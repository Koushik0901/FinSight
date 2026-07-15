# FinSight UI/UX redesign — design direction

_2026-07-14 · branch `fix/path-back-annotate-ux`_

## The identity: "Private banking, after dark"

FinSight is a **local-first, dark-first** personal-finance companion. The
redesign gives it a single coherent feeling: **calm, intelligent, warm, and
quietly premium** — a financial instrument you trust, not a generic AI
dashboard. The bones were already strong (real data, thoughtful copy, a good
net-worth chart); the pass adds **depth, material, disciplined accent, and
motion** so the whole app reads as crafted.

### Principles

1. **Depth over flatness.** The canvas is a lit room, not a void: soft ambient
   corner-glows (warm lime top-right, cool blue bottom-left) + a base vignette.
   Cards are physical surfaces — a top edge catch-light, a whisper of sheen, a
   soft grounding shadow (`--elev-1/2/3`, `--card-sheen`, `--edge-hi`).
2. **Disciplined accent.** The vivid lime (`--accent`, #C9F950) is reserved for
   the single most important thing on a surface (primary action, active nav,
   positive signal). Routine positive numbers use the calmer `--positive`.
   `--accent-soft`/`--accent-line` give lime a quiet tint for backgrounds.
3. **Typographic drama at the top, calm below.** One true display figure anchors
   each screen (the net-worth hero); everything else is quieter, with elegant
   mono eyebrows and tabular figures.
4. **Motion with intent.** Content settles in on mount (`rise-in`, staggered per
   section); cards lift on hover; the live-agent dot pulses. All fully disabled
   under `prefers-reduced-motion`.
5. **A story-shaped IA.** Navigation reads as a journey, not a flat list.

## What changed

### Foundation (`tokens.css`, `app.css`) — lifts every screen
- Material tokens (`--edge-hi`, `--card-sheen`, `--elev-1/2/3`, `--app-ambient`,
  `--accent-soft/-line`, motion easings) for both dark and light themes.
- `.card` reworked to the material recipe; `.card.hover` lifts to `--elev-2`.
- App canvas gains ambient depth (`--app-ambient` on `.main` + base vignette).
- Entrance motion: `.screen > *` rises in with a per-section stagger.

### Today — the flagship
- **Unified hero card:** net worth figure + trend + spend pace + the net-worth
  chart now live in ONE anchoring, statement-like card (was: a floating number,
  then a separate chart). `NetWorthChart` gained an `embed` prop to render flush.
- Responsive: split panels stack < 820px; the category cards use a self-
  collapsing `auto-fit` grid (fixes a pre-existing mobile horizontal overflow).

### Navigation / IA
- The flat 13-item list is regrouped into four scannable sections:
  **Overview** (Today, Inbox) · **Money** (Accounts, Budget, Categories,
  Recurring) · **Plan** (Goals, Reports, Scenarios, Path back) ·
  **Workshop** (Copilot, Rules & agents, Settings).
- Elevated active state: lime icon-chip + tinted pill + accent edge bar.
- Every route, badge, and pulse preserved; `display:contents` keeps the mobile
  2-column menu working with the new grouping.

## Validation harness (dev-only)
`ui/src/dev/mockBackend.ts` — a fixture-backed `__TAURI_INTERNALS__` that renders
the app with data in a plain browser via `?mock=rich|empty|partial|large|multi`.
Gated to `import.meta.env.DEV` + the param; tree-shaken from production; never
runs under vitest. Enables a fast visual loop and instant data-state coverage.

## Guardrails honored
- Contrast-safe ink tokens untouched; focus rings and roles preserved.
- Amounts keep `className="money"` (privacy blur) through all restructures.
- No shared class/component renames; `NetWorthChart` change is additive.
- Green gate: **74 files / 394 tests** + `tsc` clean.

## Remaining / future polish
- Deepen per-screen redesigns (Budget, Reports, Accounts) beyond the foundation lift.
- Richer empty states (the current ones are functional but plain).
- Refine the "Spent this month" category bar (still a touch busy).
- Consider genuine screen merges (Categories→Budget tab; Scenarios/Path back/Reports → an Insights hub) — deferred as higher-risk.
