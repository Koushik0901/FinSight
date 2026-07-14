# Spending Analysis Engine — design

- Date: 2026-07-14
- Status: approved (brainstorm), pending spec review → implementation plan
- Origin: user wants to understand "why did my card spend jump to ~$3k/mo when my normal is lower, and how do I get back," generalized into a robust, universal capability and made a differentiator for the Copilot.

## 1. Problem

A user's credit-card spend runs hot for a few months against their own normal. They cannot see **where** the increase went or **why**, and therefore cannot decide **how** to bring it back. The question is not "where does my money go" (a static breakdown) but "**what changed** versus my normal, is each change a blip or a new habit, and which changes are worth acting on."

The app already has adjacent pieces, and none answer this:

- `read::get_spending_breakdown` — top categories/merchants/monthly totals over a window. Static "where," no baseline, no delta, no change attribution.
- `Insights.tsx` — a shallow month-over-month "spike" card (one category, >50% vs *last* month). Not a trailing baseline, not decomposed, not ranked, single card.
- `anomaly.rs` — robust per-merchant *charge* outliers (median + MAD). Transaction-level, not period-level.
- `recurring.rs` — recurring/subscription detection. A useful input, not the analysis.

Validated against the user's real CIBC + Amex statements (2,400+ txns, throwaway probe): the increase is **broad-based lifestyle inflation across ~8 discretionary categories plus a few one-offs**, not one smoking gun; roughly half self-corrects (a travel burst, a one-time electronics buy); the genuinely recurring, reducible piece is far smaller than the headline gap; and the user's *perceived* baseline ($1.5k) sat well below their *actual* robust median (~$2.0k). A naïve "cut $X" tool would have been wrong on all counts. The engine must be honest about exactly these things.

## 2. Goals / non-goals

Goals:

- One **intelligent engine** — a persistent model of "your normal" plus a small composable analytical vocabulary over it — that multiple consumers (Copilot, a dedicated screen, Insights) all read from.
- Answer open-ended, drill-down questions ("why was May $6.6k?", "which merchants are new vs my normal?", "what doubled?", "which of these will recur?", "spike or new regime?") **conversationally, via the Copilot**, with every number computed deterministically.
- Robust and assumption-free: works for any user, income level, currency, or spending shape, degrading gracefully when history is thin.

Non-goals (v1):

- No trained/deep ML models (see §9).
- No new statement-cycle data model — calendar month is the analysis unit; a per-card statement lens is a possible later addition, out of scope here.
- No auto-actions (cancelling subscriptions, moving money). The engine informs; the user acts.
- Not personalized investment advice; this is spending analysis and organization.

## 3. Core idea

The engine is **not** a stateless "compute-and-return" tool. It is:

1. **"Your normal"** — a durable, robust profile of a user's typical spending: per-category and per-merchant median monthly baselines, a volatility band (MAD), recurring commitments, seasonality (only once ≥ ~13 months of history exists), and the user's **sticky one-off / expected / investment verdicts**. Computed in `finsight-core`, local, private.
2. **A small composable vocabulary** over that model (4 read primitives + 1 sticky write, §7) that the Copilot's existing reasoning loop orchestrates.

### The one rule that makes "intelligent" testable: the LLM does zero arithmetic

Every number a user can ask for is computed and classified in Rust and returned pre-aggregated, with a `_display` dollar string already attached (the `augment_cents_fields` helper in `reasoning/tools/mod.rs` does this today precisely because the model misdivides cents ~10–15% of the time). The Copilot only **selects** primitives and **narrates** results. If any follow-up in the acceptance tree (§11) forces the model to compute, that is a **missing primitive**, not a prompt to tune. This rule is the anti-scope-balloon guardrail: "detect all patterns" reduces to "the vocabulary must be complete over the question space," and the question space is fixed by the acceptance tree.

## 4. Invariants (the universal spine)

These hold for any user, anywhere, and are what make the engine robust rather than tuned to one case:

1. **Self-referential baselines.** Every judgment is *you vs. your own past* — never an absolute threshold, never other users. Currency-, income-, and culture-blind in one stroke. (Same principle `anomaly.rs` already uses per merchant; promoted to the whole engine.)
2. **Two-axis change decomposition** (§6): every driver of a delta carries a **mechanism** and a **persistence**; both are required.
3. **Intent is an input, not an assumption.** reduce / maintain / reallocate / observe. A target (e.g. a monthly spend ceiling) is one optional input; the default is no target (pure awareness). Stored like the existing `Assumptions` KV (`metrics.rs`).
4. **Locale-safe money.** Amounts are integer **minor units of a stated currency** (exponent varies: JPY 0, USD 2, BHD 3) — never assume "cents." Analyze and compare **within a currency**; never fabricate a cross-currency rate. Where the app currently hardcodes USD/cents (known debt from the 2026-07 math audit), the engine must not deepen it; multi-currency ledgers are decomposed per currency.
5. **Cold-start honesty — it knows what it can't say.** With < N periods it reports *composition* ("where it's going") only, and refuses *decomposition* ("what changed"), stating that it needs more history. Seasonality separation is attempted only with ≥ ~13 months; otherwise it declines rather than flag a predictable annual peak as drift.
6. **Propose → confirm → remember.** The engine cannot know *why* (a trip, a work laptop). Classifications are user-correctable and **sticky**, reusing the `V046 transfer_override` / `V010 agent_memory` pattern. This is the line between a report and something that learns your life.
7. **Reconciliation is the product.** The Copilot's numbers must always equal the screen's numbers. Guaranteed structurally by putting all computation in `finsight-core::spending` and making the agent tools and app commands thin wrappers over the same functions.

## 5. Architecture

Layering follows the existing crate boundaries (compute in core; agent tools and app commands are thin wrappers, so every surface reconciles):

```
finsight-core::spending           ← ALL computation (pure, no Tauri, deterministic, tested)
  ├─ baseline.rs    "your normal": robust per-category/merchant medians, MAD band,
  │                  recurring commitments (reuses recurring.rs), seasonality (gated)
  ├─ decompose.rs   window-vs-reference driver decomposition + two-axis tagging
  ├─ classify.rs    period classification (normal / episodic spike / regime shift)
  ├─ plan.rs        intent-driven recoverable-lever planning (honest target)
  └─ annotate.rs    read/write sticky driver verdicts (new table, §10)

finsight-agent::reasoning::tools   ← thin Tool wrappers → finsight-core::spending
  read::get_spending_baseline, explain_spending_change, classify_spending_period,
       plan_spending_reduction
  act::annotate_spending_driver
  (registered in standard_toolset() so shipped app AND finsight-eval run the same set)

finsight-app::commands::spending   ← thin #[tauri::command] wrappers → finsight-core::spending
  (feeds the thin readout in v1; the Path Back screen + Insights in fast-follow)
```

Data flow for "why was May $6.6k?": Copilot reasoning loop → `explain_spending_change(period=May)` tool → `finsight-core::spending::decompose` (loads txns via the shared `is_transfer = 0` + `non_investment_txn_predicate` filters, computes drivers against the baseline model) → ranked, two-axis-tagged drivers with `_cents`+`_display` → LLM narrates. The dedicated screen calls the same `decompose` through an app command and renders the same numbers.

## 6. The two-axis change taxonomy

Every driver of a period-vs-baseline delta gets one tag from each axis:

- **Axis 1 — mechanism (where the delta came from):**
  - `new` — a stream absent in the baseline, present now.
  - `stopped` — present in baseline, absent now (a *tailwind*; reduces spend).
  - `price_up` / `price_down` — same stream, changed average ticket.
  - `frequency_up` / `frequency_down` — same stream, changed transaction count.
  - (`price` and `frequency` may co-occur; the dominant contributor to Δ is reported, both surfaced.)
- **Axis 2 — persistence (will it repeat?):**
  - `one_off` — single/short burst unlikely to recur.
  - `recurring` — regular cadence (from `recurring.rs` + active-month count).
  - `emerging` — recurring cadence but too new to be sure (cold-start guard; e.g. a two-month-old internet bill).
  - `uncertain` — insufficient signal.

Why both are required: `new + one_off` (a $900 flight) and `new + recurring` (a $15 subscription) are the identical mechanism but opposite advice. Everything tagged `recurring`/`emerging` is a lever; everything `one_off` is already behind you. A user's sticky annotation (§10) overrides the computed persistence.

Classification rules (deterministic, robust):
- mechanism: per normalized merchant, compare recent vs baseline set membership, transaction count (frequency), and mean ticket (price); thresholds on ratios (initial: ≥1.3× for "up", set-difference for new/stopped). A one-off spike in an existing stream is `price_up + one_off`, disambiguated from a true price rise (`price_up + recurring`) by the persistence axis.
- persistence: `recurring.rs` cadence + count of distinct active months; `emerging` when a cadence looks recurring but active months < threshold; sticky annotation wins over all.

## 7. The vocabulary (4 read + 1 sticky write)

Few, well-named, composable — the LLM plans far better over 4 rich tools than a dozen narrow ones. All return `_cents` (auto-augmented with `_display`); all honor `is_transfer = 0` and `non_investment_txn_predicate`; all are per-currency.

1. **`get_spending_baseline`** *(read)* — "your normal." Robust monthly baseline overall and per category/merchant, volatility band, recurring commitments, confidence, and a `history_note` when data is thin. Answers "what's my normal?", "what are my recurring bills?".
2. **`explain_spending_change`** *(read, the workhorse)* — compare a **target window** to a **reference** (the baseline by default, or another window). Returns ranked drivers, each `{merchant/category, Δ_cents, recent_cents, base_cents, recent_count, base_count, mechanism, persistence}`. Params: `period`, optional `reference` (window or "baseline"), `filter` (`new` | `elevated` with `min_ratio` | mechanism | persistence | category), `group_by` (`merchant` | `category` | `persistence`), `limit`. This one tool covers most of the acceptance tree (§11) by parameterization.
3. **`classify_spending_period`** *(read)* — is a window `normal` / `episodic_spike` / `regime_shift`? Returns the verdict plus evidence (window total vs baseline median and MAD band; whether elevation persists across trailing months). Gated by the cold-start rule.
4. **`plan_spending_reduction`** *(read)* — given `intent` (default `reduce`) and optional `target`, return recoverable levers (recurring/emerging drivers, honoring sticky annotations) and an **honest reachable estimate**. Must not over-promise: if the recurring-reducible sum lands above a user's aspirational target, it says the remainder is *structural*, not a trimming problem.
5. **`annotate_spending_driver`** *(act, sticky write)* — mark a merchant/driver as `one_off` | `expected` | `investment` | reset. Persisted (§10), reused across every surface and future recomputes. Surfaced as a draft action in chat and as a control on the screen.

## 8. Consumers

- **Copilot (v1, the lead & the selling point).** The five tools above register in `standard_toolset()`. The existing reasoning loop orchestrates them for open-ended drill-down. This is what "Actual Budget MCP + ChatGPT" structurally cannot match (§12).
- **Thin deterministic readout (v1).** Even just a ranked-driver table (an app command rendering `explain_spending_change`), so numbers are verifiable and the chat has a surface to reconcile against.
- **Path Back screen (fast-follow).** The full experience: gap vs baseline, ranked + tagged drivers, the honest reduction plan, sticky annotations as first-class controls. Same engine.
- **Insights (fast-follow).** Replace the shallow "spike" card with `classify_spending_period` run on a schedule, surfacing a decomposition proactively when a genuine `regime_shift` is detected. Same engine.

## 9. On ML / LLMs — intelligence without the ML tax

New streams, doubled spend, spikes, regime shifts, and seasonality all come from **robust statistics** (median/MAD, set/ratio math, a light changepoint/seasonality test gated on ≥ ~13 months) **+ LLM orchestration**. This beats a trained model *here*: no training data, runs local and private, every flag traces to a number the UI can show, and no silent drift. A deep model would be worse on all four. LLM usage is confined to orchestration and narration (existing Copilot); it never computes a number. If a genuine forecasting need appears later, it earns its place then, against real data.

## 10. Data model changes

- **New migration `Vxxx__spending_driver_annotations.sql`** (next sequential version — verify highest at implementation; main is at V046, and a V047 may exist on a branch). Table keyed by normalized merchant (and/or category) holding the sticky verdict (`one_off` | `expected` | `investment`), a timestamp, and optional note. Mirrors the `transaction_transfer_override` (V046) sticky-verdict shape and the `agent_memory` (V010) durability pattern. Recompute/decomposition must **respect** this override, exactly as the categorizer respects `transfer_override`.
- **No schema change for baselines** in v1 — computed on read (cache later only if profiling demands it; a cache must never become a second source of truth).
- **Currency**: no migration, but the engine reads each account's `currency` and treats amounts as minor units of it; comparisons and sums are per-currency. (Full minor-unit/exponent correctness app-wide is tracked separately as the USD-hardcoding debt; the engine must not deepen it.)

## 11. Acceptance test — the "why was May $6.6k?" tree

Every follow-up must map to a primitive that returns the number; none may force the LLM to compute. These become `finsight-eval` cases against the sample statements (the harness runs `standard_toolset()`, so it grades the shipped agent).

| Question | Primitive call | Returns |
|---|---|---|
| Why was May $6.6k? | `explain_spending_change(period=May)` | ranked drivers, each mechanism × persistence |
| Which merchants are new vs my ~$2k months? | `explain_spending_change(period=May, filter=new)` | new streams + amounts |
| What doubled over usual? | `explain_spending_change(period=May, filter=elevated, min_ratio=2)` | merchants above their own baseline |
| How much will recur? | `explain_spending_change(period=May, group_by=persistence)` | recurring vs one-off subtotals |
| Blip or new normal? | `classify_spending_period(period=May)` | spike vs regime shift + evidence |
| Compare May to April. | `explain_spending_change(period=May, reference=April)` | driver-level delta of two windows |
| How do I get back? | `plan_spending_reduction(intent=reduce)` | recurring levers + honest reachable target |
| That Flair burst was one-time. | `annotate_spending_driver(Flair, one_off)` | remembered; leaves "levers" everywhere |

## 12. Why this beats "Actual Budget MCP + ChatGPT" (the moat)

Not "we have primitives, they have raw data" — a good MCP could expose primitives too. What ChatGPT + a generic data MCP structurally cannot do: carry a **consistent, persistent model of your normal** (robust baseline, recurring commitments, sticky one-off/habit verdicts) that is **identical across chat, screen, and insights**. They re-derive from rows each turn, do money-math in-context (unreliable), and contradict themselves between sessions. Two hard requirements capture the moat:

1. The Copilot's numbers always equal the screen's numbers (structural: one core computes both).
2. It learns your life via propose → confirm → remember (sticky annotations) — untouchable by stateless chat.

## 13. Testing & eval

- **Unit tests** (`finsight-core::spending`): mechanism and persistence classification on constructed ledgers; robust baseline resists a hot-month poisoning the median; cold-start returns composition-only and declines decomposition; per-currency isolation; sticky annotation overrides computed persistence. Mirrors `anomaly.rs` test style.
- **Reconciliation test**: the app command and the agent tool return identical figures for the same window (guards the moat requirement).
- **Eval** (`finsight-eval`): the §11 tree + follow-ups as cases against the sample statements; iterate on tiny subsets (harness-only) before any full judged run.
- **Live validation**: drive the Copilot in the real app on the sample data and confirm the drill-down answers match the deterministic readout.

## 14. Scope

- **v1**: `finsight-core::spending` (baseline, decompose, classify, plan, annotate) + the 5 Copilot tools + the annotations migration + the thin ranked-driver readout + eval cases from §11.
- **Fast-follow**: the Path Back screen; proactive Insights via `classify_spending_period`. Both on the same engine.
- `plan_spending_reduction` stays honest: the recurring-reducible piece may land above the user's aspirational target, and the tool says the rest is structural rather than inventing a number.

## 15. Open questions / risks

- **Merchant normalization quality.** Decomposition rides on clustering `AMZN MKTP CA*…` → "Amazon." Reuse the app's existing `merchant.rs` normalization; note where the throwaway probe's clustering strained (a large uncategorized long tail) and confirm the production normalizer collapses it before trusting per-merchant deltas.
- **Baseline window defaults.** Trailing 12 months, most-recent-full-month anchor (matching `reports.rs` data-anchoring), excluding the target window from its own baseline. Confirm defaults during planning.
- **Threshold tuning** (ratios for price/frequency-up, MAD multiple for regime vs spike, `emerging` month count) — start with the `anomaly.rs`-style conservative constants; treat as tunable, covered by tests.
- **Category vs merchant grain** for `plan_spending_reduction` levers — likely merchant-level with category rollups; validate against real data during planning.

## 16. Acceptance criteria

- Every §11 question answered with the LLM doing zero arithmetic (verified in eval + live).
- Screen and Copilot reconcile to the identical number for the same window (test).
- Cold-start and per-currency honesty verified by unit tests.
- A sticky annotation set in one surface changes the answer in all surfaces on the next call.
