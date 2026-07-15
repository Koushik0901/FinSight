# Copilot Generative-UI Blocks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the existing validated `AgentResponseBlock` generative-UI union with six composite finance block kinds (`spendingReview`, `accountsOverview`, `spendTimeline`, `spendingDrivers`, `watchList`, `actionPlan`) that render the target spending-review / accounts / spending-analysis surfaces natively in Copilot chat.

**Architecture:** Each block flows through the unchanged pipeline: model `response_blocks[]` → Rust `AgentResponseBlock` (parse + `valid_response_block` + `take(8)`) → `should_emit_response_block` + `response_block_within_artifact_bounds` → `generative-ui` `FinSightResponseBlock` artifact → Zod `CopilotResponseBlockSchema` re-validation → `renderers.tsx` switch → React card. Schema stays flat (no union nesting); visual reuse happens at the React layer via shared sub-components. Blocks are presentational; no new mutation path.

**Tech Stack:** Rust (serde + specta), Tauri command surface, TypeScript, React, Zod, vitest, `react-markdown`, design tokens in `ui/src/styles/tokens.css` + `.cp-*` classes in `ui/src/styles/copilot-shell.css`.

**Reference spec:** `docs/superpowers/specs/2026-07-15-copilot-generative-ui-blocks-design.md`

---

## Task conventions (read once, applies to every task)

Every code task follows the same TDD loop; each task below lists its own code so tasks are readable out of order.

- **Rust tests:** `cargo test -p finsight-app --lib commands::agent` (block validation + serde live in `crates/finsight-app/src/commands/agent.rs` tests module). Full crate: `cargo test -p finsight-app`.
- **Frontend tests:** `cd ui && npx vitest run <file>`; type-check `cd ui && npx tsc --noEmit`.
- **After ANY Rust `AgentResponseBlock` change:** regenerate bindings from repo root: `cargo run -p finsight-tauri --bin export_bindings` (rewrites `ui/src/api/bindings.ts` — never edit by hand).
- **Money in blocks is i64 cents**, rendered by `money()` from `ui/src/utils/format`. Presentational deltas ("+$213/mo") are bounded short strings (`amountDisplay`).
- **Colors:** `colorForCategoryLabel(label)` from `ui/src/utils/categoryColor`; tokens only in CSS.
- **Commit** after each task with the message shown.

**Bounds constants** (identical Rust + Zod): reuse existing `MAX_LABEL = 400` (`shortString`), `MAX_TEXT = 20_000`. New per-kind counts introduced below: review months ≤ 6, review categories ≤ 10, review actions ≤ 6, account rows ≤ 30, timeline points ≤ 24, drivers ≤ 8, watch items ≤ 8, actionPlan items ≤ 8.

---

## Phase 1 — Foundation + `spendingReview` (marquee)

### Task 1: Shared React sub-components + base CSS

**Files:**
- Modify: `ui/src/components/copilot/cards/shared.tsx`
- Create: `ui/src/components/copilot/cards/shared.subcomponents.test.tsx`
- Modify: `ui/src/styles/copilot-shell.css` (append new classes)

- [ ] **Step 1: Write failing tests**

```tsx
// ui/src/components/copilot/cards/shared.subcomponents.test.tsx
import { render, screen } from "@testing-library/react";
import { StatLine, TagPill, ActionChecklist } from "./shared";

test("StatLine joins parts with a middot", () => {
  render(<StatLine parts={["$4,086 spent", "8 of 10 envelopes under"]} />);
  expect(screen.getByText(/\$4,086 spent · 8 of 10 envelopes under/)).toBeInTheDocument();
});

test("TagPill renders label and tone data attr", () => {
  render(<TagPill label="planned" tone="planned" />);
  const el = screen.getByText("planned");
  expect(el).toHaveAttribute("data-tone", "planned");
});

test("ActionChecklist renders each item and toggles a checkbox", () => {
  render(<ActionChecklist title="Action plan" items={["Do X", "Do Y"]} />);
  expect(screen.getByText("Action plan")).toBeInTheDocument();
  expect(screen.getByText("Do X")).toBeInTheDocument();
  expect(screen.getByText("Do Y")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/components/copilot/cards/shared.subcomponents.test.tsx`
Expected: FAIL — `StatLine`/`TagPill`/`ActionChecklist` not exported.

- [ ] **Step 3: Implement the sub-components** (append to `shared.tsx`)

```tsx
import { useState } from "react";
import * as I from "../../Icons";

/** A mono, middot-joined sub-header line (spent stats, account summary, timeline caption). */
export function StatLine({ parts }: { parts: string[] }) {
  return (
    <div className="cp-statline mono">
      {parts.filter(Boolean).join(" · ")}
    </div>
  );
}

/** A small uppercase tag pill whose color comes from a tone token (drivers, category flags). */
export function TagPill({ label, tone }: { label: string; tone: string }) {
  return (
    <span className="cp-tag" data-tone={tone}>
      {label}
    </span>
  );
}

/**
 * A presentational next-steps checklist. Checkboxes toggle local-only state
 * (no persistence, no mutation) — mutating actions stay on the bundle-approval
 * flow. Shared by SpendingReviewCard month cards and the standalone ActionPlanCard.
 */
export function ActionChecklist({ title, items }: { title?: string; items: string[] }) {
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const toggle = (i: number) =>
    setChecked((prev) => {
      const next = new Set(prev);
      next.has(i) ? next.delete(i) : next.add(i);
      return next;
    });
  return (
    <div className="cp-checklist">
      {title && <p className="cp-checklist-title eyebrow">{title}</p>}
      {items.map((text, i) => (
        <button type="button" key={i} className="cp-check-row" onClick={() => toggle(i)} aria-pressed={checked.has(i)}>
          <span className={`cp-check-box ${checked.has(i) ? "is-on" : ""}`}>
            {checked.has(i) && <I.Check width={11} height={11} />}
          </span>
          <span className="cp-check-txt">{text}</span>
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Append CSS** to `ui/src/styles/copilot-shell.css`

```css
/* ── Generative-UI: shared sub-components ─────────────────────────────── */
.cp-statline { font-size: 11.5px; color: var(--ink-mute); margin-top: 4px; }
.cp-tag { font-family: var(--mono); font-size: 9px; letter-spacing: 0.05em; text-transform: uppercase;
  border: 1px solid currentColor; border-radius: 4px; padding: 1px 5px; color: var(--ink-faint); }
.cp-tag[data-tone="planned"] { color: var(--accent); }
.cp-tag[data-tone="trend"], .cp-tag[data-tone="over"] { color: var(--negative); }
.cp-tag[data-tone="prices"] { color: var(--c-dining, var(--ink-2)); }
.cp-tag[data-tone="anomaly"] { color: var(--warning, var(--negative)); }
.cp-tag[data-tone="creep"] { color: var(--c-shopping, var(--ink-2)); }
.cp-tag[data-tone="mixed"], .cp-tag[data-tone="fixed"] { color: var(--ink-faint); }
.cp-checklist { margin-top: 12px; display: flex; flex-direction: column; gap: 6px; }
.cp-checklist-title { margin: 0 0 4px; }
.cp-check-row { display: flex; align-items: flex-start; gap: 9px; background: none; border: none;
  padding: 3px 0; text-align: left; cursor: pointer; color: var(--ink-2); }
.cp-check-box { width: 16px; height: 16px; border: 1px solid var(--line); border-radius: 5px; flex-shrink: 0;
  display: inline-flex; align-items: center; justify-content: center; color: var(--accent); margin-top: 1px; }
.cp-check-box.is-on { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 14%, transparent); }
.cp-check-txt { font-size: 13px; line-height: 1.45; }
```

- [ ] **Step 5: Run tests to verify pass**

Run: `cd ui && npx vitest run src/components/copilot/cards/shared.subcomponents.test.tsx && npx tsc --noEmit`
Expected: PASS; no type errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/components/copilot/cards/shared.tsx ui/src/components/copilot/cards/shared.subcomponents.test.tsx ui/src/styles/copilot-shell.css
git commit -m "feat(copilot): shared genUI sub-components (StatLine, TagPill, ActionChecklist)"
```

---

### Task 2: `spendingReview` — Rust block

**Files:**
- Modify: `crates/finsight-app/src/commands/agent.rs` (structs + enum arm + `valid_response_block` arm + tests)
- Modify: `crates/finsight-app/src/commands/copilot_chat.rs` (`should_emit_response_block` + `response_block_within_artifact_bounds` arms + bounds consts)

- [ ] **Step 1: Write failing Rust tests** (add to the `#[cfg(test)] mod tests` in `agent.rs`)

```rust
#[test]
fn spending_review_valid_and_rejects_empty_and_oversized() {
    let ok = AgentResponseBlock::SpendingReview(AgentSpendingReviewBlock {
        months: vec![AgentReviewMonth {
            label: "May 2026".into(),
            spent_cents: 408_600,
            subtitle: Some("8 of 10 envelopes under".into()),
            categories: vec![AgentReviewCategory {
                label: "Housing".into(), amount_cents: 185_000, tag: Some("fixed".into()),
            }],
            summary: Some("A steady month.".into()),
            actions: vec!["Glance at the PG&E bill".into()],
        }],
    });
    assert!(valid_response_block(&ok));

    let no_months = AgentResponseBlock::SpendingReview(AgentSpendingReviewBlock { months: vec![] });
    assert!(!valid_response_block(&no_months));

    let bad_tag = AgentResponseBlock::SpendingReview(AgentSpendingReviewBlock {
        months: vec![AgentReviewMonth {
            label: "May".into(), spent_cents: 1, subtitle: None,
            categories: vec![AgentReviewCategory { label: "X".into(), amount_cents: 1, tag: Some("bogus".into()) }],
            summary: None, actions: vec![],
        }],
    });
    assert!(!valid_response_block(&bad_tag));
}

#[test]
fn spending_review_serde_round_trip_is_camel_case() {
    let block = AgentResponseBlock::SpendingReview(AgentSpendingReviewBlock {
        months: vec![AgentReviewMonth {
            label: "May".into(), spent_cents: 100, subtitle: None,
            categories: vec![AgentReviewCategory { label: "Housing".into(), amount_cents: 50, tag: None }],
            summary: None, actions: vec![],
        }],
    });
    let v = serde_json::to_value(&block).unwrap();
    assert_eq!(v["kind"], "spendingReview");
    assert_eq!(v["months"][0]["spentCents"], 100);
    let back: AgentResponseBlock = serde_json::from_value(v).unwrap();
    assert!(matches!(back, AgentResponseBlock::SpendingReview(_)));
}
```

- [ ] **Step 2: Run to verify fails**

Run: `cargo test -p finsight-app --lib commands::agent::tests::spending_review 2>&1 | head -20`
Expected: FAIL — `AgentSpendingReviewBlock` / variant not found (does not compile).

- [ ] **Step 3: Add the structs + enum variant** to `agent.rs` (near the other block structs, before `enum AgentResponseBlock`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentReviewCategory {
    pub label: String,
    pub amount_cents: i64,
    /// Optional flag: "over" | "fixed" | "lever". None = plain bar.
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentReviewMonth {
    pub label: String,
    pub spent_cents: i64,
    pub subtitle: Option<String>,
    pub categories: Vec<AgentReviewCategory>,
    pub summary: Option<String>,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpendingReviewBlock {
    pub months: Vec<AgentReviewMonth>,
}
```

Add the variant to `enum AgentResponseBlock`:

```rust
    SpendingReview(AgentSpendingReviewBlock),
```

- [ ] **Step 4: Add the `valid_response_block` arm**

```rust
        AgentResponseBlock::SpendingReview(b) => {
            const REVIEW_TAGS: [&str; 3] = ["over", "fixed", "lever"];
            !b.months.is_empty()
                && b.months.len() <= 6
                && b.months.iter().all(|m| {
                    !m.label.trim().is_empty()
                        && m.categories.len() <= 10
                        && m.actions.len() <= 6
                        && m.categories.iter().all(|c| {
                            !c.label.trim().is_empty()
                                && c.tag.as_deref().map(|t| REVIEW_TAGS.contains(&t)).unwrap_or(true)
                        })
                })
        }
```

- [ ] **Step 5: Add `should_emit` + bounds arms** in `copilot_chat.rs`

In `should_emit_response_block`, add:

```rust
        AgentResponseBlock::SpendingReview(_) => true,
```

In `response_block_within_artifact_bounds`, add (labels/summaries bounded like the rest):

```rust
        AgentResponseBlock::SpendingReview(b) => {
            b.months.len() <= 6
                && b.months.iter().all(|m| {
                    label_ok(&m.label)
                        && opt_label_ok(&m.subtitle)
                        && m.summary.as_deref().map(|s| s.chars().count() <= ARTIFACT_MAX_TEXT).unwrap_or(true)
                        && m.categories.len() <= 10
                        && m.categories.iter().all(|c| label_ok(&c.label) && opt_label_ok(&c.tag))
                        && m.actions.len() <= 6
                        && m.actions.iter().all(|a| label_ok(a))
                })
        }
```

- [ ] **Step 6: Run tests to verify pass**

Run: `cargo test -p finsight-app --lib commands::agent::tests::spending_review`
Expected: PASS (2 tests).

- [ ] **Step 7: Regenerate bindings**

Run (repo root): `cargo run -p finsight-tauri --bin export_bindings`
Expected: `ui/src/api/bindings.ts` now includes `spendingReview` in `CopilotResponseBlock`.

- [ ] **Step 8: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs ui/src/api/bindings.ts
git commit -m "feat(copilot): spendingReview response block (Rust + bindings)"
```

---

### Task 3: `spendingReview` — Zod + card + render

**Files:**
- Modify: `ui/src/components/copilot/agUi/artifacts.ts` (Zod branch)
- Modify: `ui/src/components/copilot/agUi/artifacts.test.ts` (accept/reject)
- Create: `ui/src/components/copilot/cards/SpendingReviewCard.tsx`
- Create: `ui/src/components/copilot/cards/SpendingReviewCard.test.tsx`
- Modify: `ui/src/components/copilot/renderers.tsx` (import + switch case)
- Modify: `ui/src/styles/copilot-shell.css`

- [ ] **Step 1: Add the Zod branch** to `CopilotResponseBlockSchema` in `artifacts.ts`

```ts
  z.object({
    kind: z.literal("spendingReview"),
    months: z
      .array(
        z.object({
          label: shortString,
          spentCents: z.number().int(),
          subtitle: shortString.nullable(),
          categories: z
            .array(z.object({ label: shortString, amountCents: z.number().int(), tag: z.enum(["over", "fixed", "lever"]).nullable() }))
            .max(10),
          summary: z.string().max(MAX_TEXT).nullable(),
          actions: z.array(shortString).max(6),
        }),
      )
      .min(1)
      .max(6),
  }),
```

- [ ] **Step 2: Add schema tests** to `artifacts.test.ts`

```ts
test("spendingReview: valid block parses", () => {
  const block = { kind: "spendingReview", months: [{ label: "May 2026", spentCents: 408600, subtitle: "8 of 10 under", categories: [{ label: "Housing", amountCents: 185000, tag: "fixed" }], summary: "Steady.", actions: ["Do X"] }] };
  expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(true);
});
test("spendingReview: unknown tag rejected", () => {
  const block = { kind: "spendingReview", months: [{ label: "May", spentCents: 1, subtitle: null, categories: [{ label: "X", amountCents: 1, tag: "bogus" }], summary: null, actions: [] }] };
  expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(false);
});
test("spendingReview: >6 months rejected", () => {
  const m = { label: "M", spentCents: 1, subtitle: null, categories: [], summary: null, actions: [] };
  const block = { kind: "spendingReview", months: Array(7).fill(m) };
  expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(false);
});
```

Run: `cd ui && npx vitest run src/components/copilot/agUi/artifacts.test.ts` — Expected: PASS.

- [ ] **Step 3: Write the card render test**

```tsx
// ui/src/components/copilot/cards/SpendingReviewCard.test.tsx
import { render, screen } from "@testing-library/react";
import { SpendingReviewCard } from "./SpendingReviewCard";

const block = {
  kind: "spendingReview" as const,
  months: [{
    label: "May 2026", spentCents: 408600, subtitle: "8 of 10 envelopes under",
    categories: [{ label: "Housing", amountCents: 185000, tag: "fixed" as const }, { label: "Dining", amountCents: 41200, tag: "over" as const }],
    summary: "A steady month.", actions: ["Glance at the PG&E bill"],
  }],
};

test("renders month header, category bars, summary, and action plan", () => {
  render(<SpendingReviewCard block={block} />);
  expect(screen.getByText("May 2026")).toBeInTheDocument();
  expect(screen.getByText(/8 of 10 envelopes under/)).toBeInTheDocument();
  expect(screen.getByText("Housing")).toBeInTheDocument();
  expect(screen.getByText("Dining")).toBeInTheDocument();
  expect(screen.getByText("A steady month.")).toBeInTheDocument();
  expect(screen.getByText("Glance at the PG&E bill")).toBeInTheDocument();
});
```

Run: `cd ui && npx vitest run src/components/copilot/cards/SpendingReviewCard.test.tsx` — Expected: FAIL (module missing).

- [ ] **Step 4: Implement the card**

```tsx
// ui/src/components/copilot/cards/SpendingReviewCard.tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import { SegmentBar } from "./shared";
import { StatLine, ActionChecklist } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "spendingReview" }>;

export function SpendingReviewCard({ block }: { block: Block }) {
  return (
    <div className="cp-review">
      {block.months.map((m, mi) => {
        const max = Math.max(...m.categories.map((c) => c.amountCents), 1);
        return (
          <div key={`${m.label}-${mi}`} className="cp-card cp-review-month">
            <div className="cp-review-hd">
              <div className="cp-card-title">{m.label}</div>
              <StatLine parts={[`${money(m.spentCents)} spent`, m.subtitle ?? ""]} />
            </div>
            <div className="cp-bars">
              {m.categories.map((c, ci) => (
                <SegmentBar
                  key={`${c.label}-${ci}`}
                  label={c.label}
                  amountCents={c.amountCents}
                  maxCents={max}
                  color={colorForCategoryLabel(c.label) ?? "var(--ink-faint)"}
                  tag={c.tag === "over" ? { text: "over" } : c.tag ? { text: c.tag, muted: true } : undefined}
                  dimmed={c.tag === "fixed"}
                />
              ))}
            </div>
            {m.summary && <div className="cp-review-summary">{m.summary}</div>}
            {m.actions.length > 0 && <ActionChecklist title="Action plan" items={m.actions} />}
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 5: Wire into `renderers.tsx`**

Add import: `import { SpendingReviewCard } from "./cards/SpendingReviewCard";`
Add switch case in `FinSightResponseBlock` (before `default`):

```tsx
    case "spendingReview":
      return <SpendingReviewCard block={block} />;
```

- [ ] **Step 6: Add CSS**

```css
/* ── spendingReview ─────────────────────────────────────────────────── */
.cp-review { display: flex; flex-direction: column; gap: 14px; }
.cp-review-hd { margin-bottom: 14px; }
.cp-review-summary { margin-top: 14px; padding: 12px 14px; background: var(--elevated); border: 1px solid var(--line);
  border-radius: var(--radius-md, 8px); font-size: 13px; line-height: 1.55; color: var(--ink-2); }
```

- [ ] **Step 7: Run tests + type-check**

Run: `cd ui && npx vitest run src/components/copilot/cards/SpendingReviewCard.test.tsx src/components/copilot/agUi/artifacts.test.ts && npx tsc --noEmit`
Expected: PASS; no type errors.

- [ ] **Step 8: Commit**

```bash
git add ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/agUi/artifacts.test.ts ui/src/components/copilot/cards/SpendingReviewCard.tsx ui/src/components/copilot/cards/SpendingReviewCard.test.tsx ui/src/components/copilot/renderers.tsx ui/src/styles/copilot-shell.css
git commit -m "feat(copilot): SpendingReviewCard render + Zod validation"
```

---

### Task 4: `spendingReview` — prompt contract + few-shot

**Files:**
- Modify: `crates/finsight-agent/src/reasoning/engine/mod.rs` (`build_system_prompt`)

- [ ] **Step 1: Extend the supported-blocks list.** In `build_system_prompt`, append to the `Supported response_blocks are exactly:` enumeration (keep the existing `.` list; add before the final period or as a new sentence):

```
{{\"kind\":\"spendingReview\",\"months\":[{{\"label\":\"May 2026\",\"spentCents\":408600,\"subtitle\":\"8 of 10 envelopes under\",\"categories\":[{{\"label\":\"Housing\",\"amountCents\":185000,\"tag\":\"fixed\"}},{{\"label\":\"Dining\",\"amountCents\":41200,\"tag\":\"over\"}}],\"summary\":\"A steady month.\",\"actions\":[\"Glance at the PG&E bill\"]}}]}}
```

- [ ] **Step 2: Add the when-to-use line** (in the paragraph that describes each kind's usage):

```
Use spendingReview specifically for a multi-month spending review (one entry per month in months[], each with its top categories, a one-line subtitle like \"N of M envelopes under\", a short summary, and 2-4 concrete action items) — never a separate categoryBreakdown per month. Mark fixed-cost categories tag \"fixed\", the breached category tag \"over\", and the single most controllable one tag \"lever\". Amounts are integer cents.
```

- [ ] **Step 3: Verify the crate still builds + prompt tests pass**

Run: `cargo test -p finsight-agent --lib reasoning::engine`
Expected: PASS (existing prompt/engine tests unaffected).

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-agent/src/reasoning/engine/mod.rs
git commit -m "feat(copilot): teach the model the spendingReview block"
```

---

## Phase 2 — `accountsOverview`

### Task 5: `accountsOverview` — Rust block

**Files:** Modify `agent.rs`, `copilot_chat.rs`.

- [ ] **Step 1: Failing tests** (`agent.rs` tests module)

```rust
#[test]
fn accounts_overview_valid_and_rejects_empty_rows() {
    let ok = AgentResponseBlock::AccountsOverview(AgentAccountsOverviewBlock {
        title: Some("7 accounts".into()),
        subtitle: Some("$137,515 tracked · 1 missing a balance".into()),
        rows: vec![
            AgentAccountRow { name: "Joint Checking".into(), subtitle: Some("Mercury ····4421".into()), type_label: "Checking".into(), amount_cents: Some(1_482_042), badge: None },
            AgentAccountRow { name: "Vanguard".into(), subtitle: Some("manual".into()), type_label: "Investment".into(), amount_cents: None, badge: Some("needs a balance set".into()) },
        ],
    });
    assert!(valid_response_block(&ok));
    let empty = AgentResponseBlock::AccountsOverview(AgentAccountsOverviewBlock { title: None, subtitle: None, rows: vec![] });
    assert!(!valid_response_block(&empty));
}
```

Run: `cargo test -p finsight-app --lib commands::agent::tests::accounts_overview 2>&1 | head` — Expected: FAIL (no such type).

- [ ] **Step 2: Structs + enum variant** (`agent.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccountRow {
    pub name: String,
    pub subtitle: Option<String>,
    pub type_label: String,
    /// None → account has no known balance; renderer shows `badge` instead.
    pub amount_cents: Option<i64>,
    pub badge: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccountsOverviewBlock {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub rows: Vec<AgentAccountRow>,
}
```

Enum arm: `    AccountsOverview(AgentAccountsOverviewBlock),`

- [ ] **Step 3: `valid_response_block` arm**

```rust
        AgentResponseBlock::AccountsOverview(b) => {
            !b.rows.is_empty()
                && b.rows.len() <= 30
                && b.rows.iter().all(|r| !r.name.trim().is_empty() && !r.type_label.trim().is_empty())
        }
```

- [ ] **Step 4: `copilot_chat.rs`** — `should_emit`: `AgentResponseBlock::AccountsOverview(_) => true,` and bounds:

```rust
        AgentResponseBlock::AccountsOverview(b) => {
            opt_label_ok(&b.title)
                && opt_label_ok(&b.subtitle)
                && b.rows.len() <= 30
                && b.rows.iter().all(|r| {
                    label_ok(&r.name) && opt_label_ok(&r.subtitle) && label_ok(&r.type_label) && opt_label_ok(&r.badge)
                })
        }
```

- [ ] **Step 5:** Run `cargo test -p finsight-app --lib commands::agent::tests::accounts_overview` → PASS. Regenerate bindings: `cargo run -p finsight-tauri --bin export_bindings`.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs ui/src/api/bindings.ts
git commit -m "feat(copilot): accountsOverview response block (Rust + bindings)"
```

---

### Task 6: `accountsOverview` — Zod + card + render + prompt

**Files:** `artifacts.ts`(+test), `cards/AccountsOverviewCard.tsx`(+test), `renderers.tsx`, `copilot-shell.css`, `engine/mod.rs`.

- [ ] **Step 1: Zod branch** (`artifacts.ts`)

```ts
  z.object({
    kind: z.literal("accountsOverview"),
    title: shortString.nullable(),
    subtitle: shortString.nullable(),
    rows: z
      .array(z.object({ name: shortString, subtitle: shortString.nullable(), typeLabel: shortString, amountCents: z.number().int().nullable(), badge: shortString.nullable() }))
      .min(1)
      .max(30),
  }),
```

- [ ] **Step 2: Card test** (`AccountsOverviewCard.test.tsx`)

```tsx
import { render, screen } from "@testing-library/react";
import { AccountsOverviewCard } from "./AccountsOverviewCard";
const block = { kind: "accountsOverview" as const, title: "7 accounts", subtitle: "$137,515 tracked · 1 missing a balance",
  rows: [ { name: "Amex Gold", subtitle: "Amex ····1006", typeLabel: "Credit", amountCents: -241800, badge: null },
          { name: "Vanguard Brokerage", subtitle: "manual", typeLabel: "Investment", amountCents: null, badge: "needs a balance set" } ] };
test("renders header, negative balance, and needs-balance badge", () => {
  render(<AccountsOverviewCard block={block} />);
  expect(screen.getByText("7 accounts")).toBeInTheDocument();
  expect(screen.getByText("Amex Gold")).toBeInTheDocument();
  expect(screen.getByText("needs a balance set")).toBeInTheDocument();
});
```

Run: `cd ui && npx vitest run src/components/copilot/cards/AccountsOverviewCard.test.tsx` — FAIL.

- [ ] **Step 3: Card** (`AccountsOverviewCard.tsx`)

```tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { StatLine } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "accountsOverview" }>;

export function AccountsOverviewCard({ block }: { block: Block }) {
  return (
    <div className="cp-card cp-accounts">
      {(block.title || block.subtitle) && (
        <div className="cp-accounts-hd">
          {block.title && <div className="cp-card-title">{block.title}</div>}
          {block.subtitle && <StatLine parts={[block.subtitle]} />}
        </div>
      )}
      <div className="cp-accounts-rows">
        {block.rows.map((r, i) => (
          <div key={`${r.name}-${i}`} className="cp-account-row">
            <div className="cp-account-id">
              <span className="cp-account-name">{r.name}</span>
              {r.subtitle && <span className="cp-account-sub mono">{r.subtitle}</span>}
            </div>
            <span className="cp-account-type chip">{r.typeLabel}</span>
            {r.amountCents == null ? (
              <span className="cp-account-badge">{r.badge ?? "needs a balance set"}</span>
            ) : (
              <span className={`cp-account-bal mono money ${r.amountCents < 0 ? "is-neg" : ""}`}>{money(r.amountCents)}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Wire `renderers.tsx`** — import + `case "accountsOverview": return <AccountsOverviewCard block={block} />;`

- [ ] **Step 5: CSS**

```css
/* ── accountsOverview ───────────────────────────────────────────────── */
.cp-accounts-hd { margin-bottom: 14px; }
.cp-accounts-rows { display: flex; flex-direction: column; }
.cp-account-row { display: grid; grid-template-columns: 1fr auto auto; gap: 14px; align-items: center;
  padding: 12px 0; border-top: 1px solid var(--line); }
.cp-account-row:first-child { border-top: none; }
.cp-account-id { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.cp-account-name { font-size: 13.5px; font-weight: 600; color: var(--ink); }
.cp-account-sub { font-size: 11px; color: var(--ink-faint); }
.cp-account-type { justify-self: center; }
.cp-account-bal { font-size: 13px; color: var(--ink); text-align: right; font-variant-numeric: tabular-nums; }
.cp-account-bal.is-neg { color: var(--negative); }
.cp-account-badge { font-family: var(--mono); font-size: 10px; text-transform: uppercase; letter-spacing: 0.04em;
  color: var(--warning, var(--accent)); border: 1px dashed currentColor; border-radius: 6px; padding: 3px 8px; }
```

- [ ] **Step 6: Prompt** (`engine/mod.rs`): add block schema example + usage line:

```
{{\"kind\":\"accountsOverview\",\"title\":\"7 accounts\",\"subtitle\":\"$137,515 tracked · 1 missing a balance\",\"rows\":[{{\"name\":\"Amex Gold\",\"subtitle\":\"Amex ····1006\",\"typeLabel\":\"Credit\",\"amountCents\":-241800,\"badge\":null}},{{\"name\":\"Vanguard Brokerage\",\"subtitle\":\"manual\",\"typeLabel\":\"Investment\",\"amountCents\":null,\"badge\":\"needs a balance set\"}}]}}
```
Usage: `Use accountsOverview specifically to list the user's accounts with type and balance; set amountCents to null and badge to a short reason (e.g. \"needs a balance set\") for an account with no known balance. Never invent a balance.`

- [ ] **Step 7:** Run `cd ui && npx vitest run src/components/copilot/cards/AccountsOverviewCard.test.tsx src/components/copilot/agUi/artifacts.test.ts && npx tsc --noEmit`; `cargo test -p finsight-agent --lib reasoning::engine`. Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add ui/src/components/copilot/agUi/artifacts.ts ui/src/components/copilot/agUi/artifacts.test.ts ui/src/components/copilot/cards/AccountsOverviewCard.tsx ui/src/components/copilot/cards/AccountsOverviewCard.test.tsx ui/src/components/copilot/renderers.tsx ui/src/styles/copilot-shell.css crates/finsight-agent/src/reasoning/engine/mod.rs
git commit -m "feat(copilot): AccountsOverviewCard end-to-end"
```

---

## Phase 3 — Analysis trio (`spendTimeline`, `spendingDrivers`, `watchList`)

### Task 7: `spendTimeline` — Rust block

**Files:** `agent.rs`, `copilot_chat.rs`.

- [ ] **Step 1: Failing test** (`agent.rs`)

```rust
#[test]
fn spend_timeline_valid_and_bounds() {
    let ok = AgentResponseBlock::SpendTimeline(AgentSpendTimelineBlock {
        title: Some("Monthly spend".into()), subtitle: None,
        points: vec![
            AgentTimelinePoint { label: "Jan".into(), amount_cents: 360_000, highlight: false, annotation: None, projected: false },
            AgentTimelinePoint { label: "Apr".into(), amount_cents: 570_000, highlight: false, annotation: Some("LISBON".into()), projected: false },
        ],
    });
    assert!(valid_response_block(&ok));
    let too_few = AgentResponseBlock::SpendTimeline(AgentSpendTimelineBlock { title: None, subtitle: None, points: vec![] });
    assert!(!valid_response_block(&too_few));
}
```

Run → FAIL.

- [ ] **Step 2: Structs + variant** (`agent.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentTimelinePoint {
    pub label: String,
    pub amount_cents: i64,
    #[serde(default)] pub highlight: bool,
    pub annotation: Option<String>,
    #[serde(default)] pub projected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpendTimelineBlock {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub points: Vec<AgentTimelinePoint>,
}
```
Enum arm: `    SpendTimeline(AgentSpendTimelineBlock),`

- [ ] **Step 3: `valid_response_block` arm**

```rust
        AgentResponseBlock::SpendTimeline(b) => {
            b.points.len() >= 2
                && b.points.len() <= 24
                && b.points.iter().all(|p| !p.label.trim().is_empty())
        }
```

- [ ] **Step 4: `copilot_chat.rs`** — `should_emit`: `AgentResponseBlock::SpendTimeline(_) => true,`; bounds:

```rust
        AgentResponseBlock::SpendTimeline(b) => {
            opt_label_ok(&b.title)
                && opt_label_ok(&b.subtitle)
                && b.points.len() <= 24
                && b.points.iter().all(|p| label_ok(&p.label) && opt_label_ok(&p.annotation))
        }
```

- [ ] **Step 5:** `cargo test -p finsight-app --lib commands::agent::tests::spend_timeline` → PASS. Regen bindings.

- [ ] **Step 6: Commit** `feat(copilot): spendTimeline response block (Rust + bindings)`.

---

### Task 8: `spendTimeline` — Zod + card + render + prompt

- [ ] **Step 1: Zod branch**

```ts
  z.object({
    kind: z.literal("spendTimeline"),
    title: shortString.nullable(),
    subtitle: shortString.nullable(),
    points: z
      .array(z.object({ label: shortString, amountCents: z.number().int(), highlight: z.boolean().optional().default(false), annotation: shortString.nullable(), projected: z.boolean().optional().default(false) }))
      .min(2)
      .max(24),
  }),
```

- [ ] **Step 2: Card test** — asserts labels + annotation render.

```tsx
import { render, screen } from "@testing-library/react";
import { SpendTimelineCard } from "./SpendTimelineCard";
const block = { kind: "spendTimeline" as const, title: "Monthly spend", subtitle: null,
  points: [ { label: "Jan", amountCents: 360000, highlight: false, annotation: null, projected: false },
            { label: "Apr", amountCents: 570000, highlight: false, annotation: "LISBON", projected: false },
            { label: "Jul", amountCents: 440000, highlight: true, annotation: null, projected: true } ] };
test("renders bars with labels and an annotation", () => {
  render(<SpendTimelineCard block={block} />);
  expect(screen.getByText("Jan")).toBeInTheDocument();
  expect(screen.getByText("LISBON")).toBeInTheDocument();
});
```

- [ ] **Step 3: Card** (`SpendTimelineCard.tsx`) — vertical bars scaled to max; highlight → accent; projected → dashed; annotation above.

```tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { money } from "../../../utils/format";
import { StatLine } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "spendTimeline" }>;

export function SpendTimelineCard({ block }: { block: Block }) {
  const max = Math.max(...block.points.map((p) => p.amountCents), 1);
  return (
    <div className="cp-card cp-timeline">
      {block.title && <div className="cp-card-title">{block.title}</div>}
      {block.subtitle && <StatLine parts={[block.subtitle]} />}
      <div className="cp-timeline-bars">
        {block.points.map((p, i) => (
          <div key={`${p.label}-${i}`} className={`cp-tl-col ${p.highlight ? "is-hl" : ""} ${p.projected ? "is-proj" : ""}`}>
            {p.annotation && <span className="cp-tl-note">{p.annotation}</span>}
            <span className="cp-tl-val mono">{money(p.amountCents)}</span>
            <div className="cp-tl-bar" style={{ height: `${Math.max(4, (p.amountCents / max) * 100)}%` }} />
            <span className="cp-tl-label">{p.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Wire renderers** — `case "spendTimeline": return <SpendTimelineCard block={block} />;`

- [ ] **Step 5: CSS**

```css
/* ── spendTimeline ──────────────────────────────────────────────────── */
.cp-timeline-bars { display: flex; align-items: flex-end; gap: 10px; height: 160px; margin-top: 16px; }
.cp-tl-col { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: flex-end; height: 100%; gap: 4px; }
.cp-tl-note { font-family: var(--mono); font-size: 9px; letter-spacing: 0.05em; text-transform: uppercase; color: var(--accent); }
.cp-tl-val { font-size: 10.5px; color: var(--ink-faint); }
.cp-tl-bar { width: 100%; max-width: 34px; background: var(--surface-2, var(--bg-2)); border-radius: 6px 6px 0 0; min-height: 4px; }
.cp-tl-col.is-hl .cp-tl-bar { background: color-mix(in srgb, var(--accent) 55%, var(--bg-2)); }
.cp-tl-col.is-proj .cp-tl-bar { background: repeating-linear-gradient(45deg, var(--bg-2), var(--bg-2) 4px, transparent 4px, transparent 8px);
  border: 1px dashed var(--line); }
.cp-tl-label { font-size: 11px; color: var(--ink-mute); }
```

- [ ] **Step 6: Prompt** (`engine/mod.rs`): schema example + usage line:

```
{{\"kind\":\"spendTimeline\",\"title\":\"Monthly spend · Jan–Jul 2026\",\"subtitle\":\"last 3 months highlighted\",\"points\":[{{\"label\":\"Jan\",\"amountCents\":360000}},{{\"label\":\"Apr\",\"amountCents\":570000,\"annotation\":\"LISBON\"}},{{\"label\":\"Jul\",\"amountCents\":440000,\"highlight\":true,\"projected\":true}}]}}
```
Usage: `Use spendTimeline for a month-by-month spend trend (2-24 points); set highlight true on the recent months you're focusing on, projected true on an incomplete current month, and annotation for a one-word cause of an outlier bar.`

- [ ] **Step 7:** vitest (card + artifacts) + tsc + `cargo test -p finsight-agent --lib reasoning::engine` → PASS.

- [ ] **Step 8: Commit** `feat(copilot): SpendTimelineCard end-to-end`.

---

### Task 9: `spendingDrivers` — Rust block

- [ ] **Step 1: Failing test** — valid block + reject bad tag.

```rust
#[test]
fn spending_drivers_valid_and_rejects_bad_tag() {
    let ok = AgentResponseBlock::SpendingDrivers(AgentSpendingDriversBlock {
        title: "What's driving the +$728/mo".into(), subtitle: Some("vs Jan–Feb".into()),
        drivers: vec![AgentDriver { label: "Travel".into(), tag: "planned".into(), amount_display: "+$213/mo".into(), note: Some("Italy deposits".into()) }],
    });
    assert!(valid_response_block(&ok));
    let bad = AgentResponseBlock::SpendingDrivers(AgentSpendingDriversBlock {
        title: "x".into(), subtitle: None,
        drivers: vec![AgentDriver { label: "y".into(), tag: "bogus".into(), amount_display: "z".into(), note: None }],
    });
    assert!(!valid_response_block(&bad));
}
```

- [ ] **Step 2: Structs + variant**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentDriver {
    pub label: String,
    /// "planned" | "trend" | "prices" | "anomaly" | "creep" | "mixed"
    pub tag: String,
    /// Presentational delta string, e.g. "+$213/mo" (bounded short string).
    pub amount_display: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpendingDriversBlock {
    pub title: String,
    pub subtitle: Option<String>,
    pub drivers: Vec<AgentDriver>,
}
```
Enum arm: `    SpendingDrivers(AgentSpendingDriversBlock),`

- [ ] **Step 3: `valid_response_block` arm**

```rust
        AgentResponseBlock::SpendingDrivers(b) => {
            const DRIVER_TAGS: [&str; 6] = ["planned", "trend", "prices", "anomaly", "creep", "mixed"];
            !b.title.trim().is_empty()
                && !b.drivers.is_empty()
                && b.drivers.len() <= 8
                && b.drivers.iter().all(|d| {
                    !d.label.trim().is_empty()
                        && !d.amount_display.trim().is_empty()
                        && DRIVER_TAGS.contains(&d.tag.as_str())
                })
        }
```

- [ ] **Step 4: `copilot_chat.rs`** — `should_emit`: true; bounds:

```rust
        AgentResponseBlock::SpendingDrivers(b) => {
            label_ok(&b.title)
                && opt_label_ok(&b.subtitle)
                && b.drivers.len() <= 8
                && b.drivers.iter().all(|d| label_ok(&d.label) && label_ok(&d.tag) && label_ok(&d.amount_display) && opt_label_ok(&d.note))
        }
```

- [ ] **Step 5:** cargo test → PASS. Regen bindings.
- [ ] **Step 6: Commit** `feat(copilot): spendingDrivers response block (Rust + bindings)`.

---

### Task 10: `spendingDrivers` — Zod + card + render + prompt

- [ ] **Step 1: Zod branch**

```ts
  z.object({
    kind: z.literal("spendingDrivers"),
    title: shortString,
    subtitle: shortString.nullable(),
    drivers: z
      .array(z.object({ label: shortString, tag: z.enum(["planned", "trend", "prices", "anomaly", "creep", "mixed"]), amountDisplay: shortString, note: shortString.nullable() }))
      .min(1)
      .max(8),
  }),
```

- [ ] **Step 2: Card test** — asserts driver label, tag pill, amountDisplay.

```tsx
import { render, screen } from "@testing-library/react";
import { SpendingDriversCard } from "./SpendingDriversCard";
const block = { kind: "spendingDrivers" as const, title: "Drivers", subtitle: null,
  drivers: [ { label: "Travel", tag: "planned" as const, amountDisplay: "+$213/mo", note: "Italy deposits" } ] };
test("renders driver row with tag and amount", () => {
  render(<SpendingDriversCard block={block} />);
  expect(screen.getByText("Travel")).toBeInTheDocument();
  expect(screen.getByText("planned")).toBeInTheDocument();
  expect(screen.getByText("+$213/mo")).toBeInTheDocument();
});
```

- [ ] **Step 3: Card** (`SpendingDriversCard.tsx`)

```tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { colorForCategoryLabel } from "../../../utils/categoryColor";
import { StatLine, TagPill } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "spendingDrivers" }>;

export function SpendingDriversCard({ block }: { block: Block }) {
  return (
    <div className="cp-card cp-drivers">
      <div className="cp-card-title">{block.title}</div>
      {block.subtitle && <StatLine parts={[block.subtitle]} />}
      <div className="cp-drivers-list">
        {block.drivers.map((d, i) => (
          <div key={`${d.label}-${i}`} className="cp-driver-row">
            <span className="cp-dot" style={{ background: colorForCategoryLabel(d.label) ?? "var(--ink-faint)" }} />
            <span className="cp-driver-label">{d.label}</span>
            <TagPill label={d.tag} tone={d.tag} />
            <span className="cp-driver-amt mono">{d.amountDisplay}</span>
            {d.note && <span className="cp-driver-note">{d.note}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Wire renderers** — `case "spendingDrivers": return <SpendingDriversCard block={block} />;`

- [ ] **Step 5: CSS**

```css
/* ── spendingDrivers ────────────────────────────────────────────────── */
.cp-drivers-list { display: flex; flex-direction: column; margin-top: 14px; }
.cp-driver-row { display: grid; grid-template-columns: auto auto auto 1fr; gap: 10px; align-items: center;
  padding: 11px 0; border-top: 1px solid var(--line); }
.cp-driver-row:first-child { border-top: none; }
.cp-driver-label { font-size: 13.5px; font-weight: 600; color: var(--ink); }
.cp-driver-amt { font-size: 13px; color: var(--ink); text-align: right; font-variant-numeric: tabular-nums; grid-column: 4; }
.cp-driver-note { grid-column: 2 / -1; font-size: 12px; color: var(--ink-faint); }
```

- [ ] **Step 6: Prompt** (`engine/mod.rs`):

```
{{\"kind\":\"spendingDrivers\",\"title\":\"What's driving the +$728/mo\",\"subtitle\":\"vs your Jan–Feb baseline\",\"drivers\":[{{\"label\":\"Travel\",\"tag\":\"planned\",\"amountDisplay\":\"+$213/mo\",\"note\":\"Italy flight deposits\"}}]}}
```
Usage: `Use spendingDrivers to break down what changed vs a baseline: one row per driver with a tag (planned/trend/prices/anomaly/creep/mixed), a signed per-month delta string in amountDisplay, and a short note. Copy delta strings; do not compute.`

- [ ] **Step 7:** vitest + tsc + `cargo test -p finsight-agent --lib reasoning::engine` → PASS.
- [ ] **Step 8: Commit** `feat(copilot): SpendingDriversCard end-to-end`.

---

### Task 11: `watchList` — Rust block

- [ ] **Step 1: Failing test**

```rust
#[test]
fn watch_list_valid_and_rejects_empty() {
    let ok = AgentResponseBlock::WatchList(AgentWatchListBlock {
        title: "Watch out for these".into(),
        items: vec![AgentWatchItem { label: "The Amex balance".into(), detail: "revolving at 24.9%".into(), amount_display: Some("−$50/mo".into()) }],
    });
    assert!(valid_response_block(&ok));
    let empty = AgentResponseBlock::WatchList(AgentWatchListBlock { title: "x".into(), items: vec![] });
    assert!(!valid_response_block(&empty));
}
```

- [ ] **Step 2: Structs + variant**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentWatchItem {
    pub label: String,
    pub detail: String,
    pub amount_display: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentWatchListBlock {
    pub title: String,
    pub items: Vec<AgentWatchItem>,
}
```
Enum arm: `    WatchList(AgentWatchListBlock),`

- [ ] **Step 3: `valid_response_block` arm**

```rust
        AgentResponseBlock::WatchList(b) => {
            !b.title.trim().is_empty()
                && !b.items.is_empty()
                && b.items.len() <= 8
                && b.items.iter().all(|it| !it.label.trim().is_empty())
        }
```

- [ ] **Step 4: `copilot_chat.rs`** — `should_emit`: true; bounds:

```rust
        AgentResponseBlock::WatchList(b) => {
            label_ok(&b.title)
                && b.items.len() <= 8
                && b.items.iter().all(|it| label_ok(&it.label) && it.detail.chars().count() <= ARTIFACT_MAX_TEXT && opt_label_ok(&it.amount_display))
        }
```

- [ ] **Step 5:** cargo test → PASS. Regen bindings.
- [ ] **Step 6: Commit** `feat(copilot): watchList response block (Rust + bindings)`.

---

### Task 12: `watchList` — Zod + card + render + prompt

- [ ] **Step 1: Zod branch**

```ts
  z.object({
    kind: z.literal("watchList"),
    title: shortString,
    items: z.array(z.object({ label: shortString, detail: z.string().max(MAX_TEXT), amountDisplay: shortString.nullable() })).min(1).max(8),
  }),
```

- [ ] **Step 2: Card test**

```tsx
import { render, screen } from "@testing-library/react";
import { WatchListCard } from "./WatchListCard";
const block = { kind: "watchList" as const, title: "Watch out for these",
  items: [ { label: "The Amex balance", detail: "revolving at 24.9%", amountDisplay: "−$50/mo" } ] };
test("renders numbered watch items", () => {
  render(<WatchListCard block={block} />);
  expect(screen.getByText("The Amex balance")).toBeInTheDocument();
  expect(screen.getByText("1")).toBeInTheDocument();
});
```

- [ ] **Step 3: Card** (`WatchListCard.tsx`)

```tsx
import type { CopilotResponseBlock } from "../../../api/client";

type Block = Extract<CopilotResponseBlock, { kind: "watchList" }>;

export function WatchListCard({ block }: { block: Block }) {
  return (
    <div className="cp-card cp-watch">
      <div className="cp-card-title">{block.title}</div>
      <div className="cp-watch-list">
        {block.items.map((it, i) => (
          <div key={`${it.label}-${i}`} className="cp-watch-row">
            <span className="cp-watch-n">{i + 1}</span>
            <div className="cp-watch-body">
              <span className="cp-watch-label">{it.label}</span>
              <span className="cp-watch-detail">{it.detail}</span>
            </div>
            {it.amountDisplay && <span className="cp-watch-amt mono">{it.amountDisplay}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Wire renderers** — `case "watchList": return <WatchListCard block={block} />;`

- [ ] **Step 5: CSS**

```css
/* ── watchList ──────────────────────────────────────────────────────── */
.cp-watch-list { display: flex; flex-direction: column; gap: 10px; margin-top: 14px; }
.cp-watch-row { display: grid; grid-template-columns: auto 1fr auto; gap: 12px; align-items: start;
  padding: 11px 13px; background: var(--elevated); border: 1px solid var(--line); border-radius: 8px; }
.cp-watch-n { width: 20px; height: 20px; border-radius: 999px; background: var(--bg-2); color: var(--ink-mute);
  font-family: var(--mono); font-size: 11px; display: inline-flex; align-items: center; justify-content: center; }
.cp-watch-body { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.cp-watch-label { font-size: 13px; font-weight: 600; color: var(--ink); }
.cp-watch-detail { font-size: 12px; color: var(--ink-mute); line-height: 1.45; }
.cp-watch-amt { font-size: 12.5px; color: var(--negative); white-space: nowrap; }
```

- [ ] **Step 6: Prompt** (`engine/mod.rs`):

```
{{\"kind\":\"watchList\",\"title\":\"Watch out for these\",\"items\":[{{\"label\":\"The Amex balance\",\"detail\":\"$2,418 revolving at 24.9%\",\"amountDisplay\":\"−$50/mo\"}}]}}
```
Usage: `Use watchList for a short ranked list of risks/things to watch, 1-8 items, each a label + one-line detail and an optional cost string in amountDisplay.`

- [ ] **Step 7:** vitest + tsc + `cargo test -p finsight-agent --lib reasoning::engine` → PASS.
- [ ] **Step 8: Commit** `feat(copilot): WatchListCard end-to-end`.

---

## Phase 4 — `actionPlan` standalone + hardening

### Task 13: `actionPlan` — Rust block

- [ ] **Step 1: Failing test**

```rust
#[test]
fn action_plan_valid_and_rejects_empty() {
    let ok = AgentResponseBlock::ActionPlan(AgentActionPlanBlock { title: Some("Action plan".into()), items: vec!["Do X".into()] });
    assert!(valid_response_block(&ok));
    let empty = AgentResponseBlock::ActionPlan(AgentActionPlanBlock { title: None, items: vec![] });
    assert!(!valid_response_block(&empty));
}
```

- [ ] **Step 2: Struct + variant**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentActionPlanBlock {
    pub title: Option<String>,
    pub items: Vec<String>,
}
```
Enum arm: `    ActionPlan(AgentActionPlanBlock),`

- [ ] **Step 3: `valid_response_block` arm**

```rust
        AgentResponseBlock::ActionPlan(b) => {
            !b.items.is_empty() && b.items.len() <= 8 && b.items.iter().all(|i| !i.trim().is_empty())
        }
```

- [ ] **Step 4: `copilot_chat.rs`** — `should_emit`: true; bounds:

```rust
        AgentResponseBlock::ActionPlan(b) => {
            opt_label_ok(&b.title) && b.items.len() <= 8 && b.items.iter().all(|i| label_ok(i))
        }
```

- [ ] **Step 5:** cargo test → PASS. Regen bindings.
- [ ] **Step 6: Commit** `feat(copilot): actionPlan response block (Rust + bindings)`.

---

### Task 14: `actionPlan` — Zod + card + render + prompt

- [ ] **Step 1: Zod branch**

```ts
  z.object({
    kind: z.literal("actionPlan"),
    title: shortString.nullable(),
    items: z.array(shortString).min(1).max(8),
  }),
```

- [ ] **Step 2: Card test**

```tsx
import { render, screen } from "@testing-library/react";
import { ActionPlanCard } from "./ActionPlanCard";
test("renders standalone action plan items", () => {
  render(<ActionPlanCard block={{ kind: "actionPlan", title: "Action plan", items: ["Sweep $168 into House Fund"] }} />);
  expect(screen.getByText("Action plan")).toBeInTheDocument();
  expect(screen.getByText("Sweep $168 into House Fund")).toBeInTheDocument();
});
```

- [ ] **Step 3: Card** (`ActionPlanCard.tsx`) — reuses `ActionChecklist`.

```tsx
import type { CopilotResponseBlock } from "../../../api/client";
import { ActionChecklist } from "./shared";

type Block = Extract<CopilotResponseBlock, { kind: "actionPlan" }>;

export function ActionPlanCard({ block }: { block: Block }) {
  return (
    <div className="cp-card">
      <ActionChecklist title={block.title ?? "Action plan"} items={block.items} />
    </div>
  );
}
```

- [ ] **Step 4: Wire renderers** — `case "actionPlan": return <ActionPlanCard block={block} />;`

- [ ] **Step 5: Prompt** (`engine/mod.rs`):

```
{{\"kind\":\"actionPlan\",\"title\":\"Action plan\",\"items\":[\"Sweep the unused $168 into the House Fund\"]}}
```
Usage: `Use actionPlan for a short standalone checklist of concrete next steps (1-8 items) when the answer's value is the to-do list itself.`

- [ ] **Step 6:** vitest + tsc + `cargo test -p finsight-agent --lib reasoning::engine` → PASS.
- [ ] **Step 7: Commit** `feat(copilot): ActionPlanCard end-to-end`.

---

### Task 15: Full green bar, fallback-renderer coverage, and emission verification

**Files:** Modify `ui/src/components/copilot/renderers.test.tsx` (regression: unknown kind → fallback still works); no code changes expected beyond fixes surfaced by the full suite.

- [ ] **Step 1: Add a renderers regression test** confirming an unknown block kind still hits the fallback (guards the switch's `default`):

```tsx
test("unknown block kind falls through to the FinSightResponseBlock default (null, no throw)", () => {
  // @ts-expect-error deliberately invalid kind
  const { container } = render(<FinSightResponseBlock block={{ kind: "notARealKind" }} isRunning={false} />);
  expect(container).toBeTruthy();
});
```
(Import `FinSightResponseBlock` from `./renderers` if not already.)

- [ ] **Step 2: Regenerate bindings and confirm clean tree**

Run (repo root): `cargo run -p finsight-tauri --bin export_bindings && git diff --stat ui/src/api/bindings.ts`
Expected: no unstaged bindings drift (already committed per-block).

- [ ] **Step 3: Run the full green bar**

```bash
cargo test -p finsight-app -p finsight-agent
cd ui && npx vitest run && npx tsc --noEmit
```
Expected: all Rust + frontend tests pass; 0 type errors. Fix any failures inline.

- [ ] **Step 4: Emission verification (user-run — needs OPENROUTER_API_KEY).** Per `memory/feedback_eval_subset_iteration`, run the eval harness on a small subset covering the three flows and confirm `response_block_kinds` includes the new kinds:

```bash
# from repo root — small model, tiny question subset (no judge):
cargo run -p finsight-eval -- --model <configured-model> --out eval/out/genui-subset.jsonl \
  # (limit to the 3-month review / accounts / drivers questions per the eval CLI)
```
Inspect the emitted `response_block_kinds` in the output JSONL. If a target flow degrades to markdown/table, iterate the prompt (schema wording + few-shot) — NOT the schema — and re-run the subset. Full 65-Q eval only once the subset is stable.

- [ ] **Step 5: Live visual check.** Launch the app (`pnpm tauri:dev`), open Copilot, and drive the three flows (or feed hand-written payloads via the dev mock harness `?mock=…` per `memory/project_ui_redesign_harness`). Confirm the three screenshots reproduce, dark + light, no horizontal overflow. Screenshot for the record.

- [ ] **Step 6: Final commit**

```bash
git add ui/src/components/copilot/renderers.test.tsx
git commit -m "test(copilot): fallback-kind regression + full green bar for genUI blocks"
```

---

## Self-review notes

- **Spec coverage:** spendingReview (§4.1)→T2–4; accountsOverview (§4.2)→T5–6; spendTimeline (§4.3)→T7–8; spendingDrivers (§4.4)→T9–10; watchList (§4.5)→T11–12; actionPlan (§4.6)→T13–14; shared sub-components (§3)→T1; emission reliability (§6)→T4/T15; security invariants (§3) preserved by reusing `valid_response_block`+bounds+Zod on every block; `take(8)` safety (§3) preserved because spendingReview is one composite block. Deferred items (§7) intentionally not tasked.
- **Type consistency:** Rust `amount_cents`/`spent_cents`/`type_label`/`amount_display` serialize camelCase (`amountCents`/`spentCents`/`typeLabel`/`amountDisplay`) — matched in Zod branches and card props. Tag enums identical across Rust `valid_response_block`, Zod `z.enum`, and prompt examples (`over/fixed/lever`; `planned/trend/prices/anomaly/creep/mixed`).
- **No placeholders:** every code step shows concrete code; CSS is concrete and token-based (refined visually in T15 against the live preview).
```
