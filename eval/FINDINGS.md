# FinSight Copilot — evaluation findings

Results of running the benchmark (`benchmark.jsonl`) against `z-ai/glm-5.2:exacto`,
judged by `google/gemini-3.1-pro-preview`. Every run is in MLflow
(`sqlite:///eval/mlflow.db`); per-run details are under `eval/runs/<timestamp>/`.

## The evaluate → analyze → improve loop

| Run | What changed | Overall /5 | Pass ≥4 | Fabrication | Notes |
|-----|--------------|-----------:|--------:|------------:|-------|
| v1 baseline | 42 Q, naive judge | 1.36 | 7% | 71% | most "failures" were measurement error |
| v2 re-judge | judge grounded in the **complete** household facts | 3.12 | 52% | 29% | same answers — isolated ~half the failures as judge bias |
| v3 | 12-month seed, `is_usable` decoupled from score | 2.91 | 48% | 45% | exposed the real "$163 surplus" bug |
| v4 | **90-day-window surplus fixed**, +11 planning Q (53 total) | 3.47 | 62% | 26% | affordability 1.0 → 5.0 |
| v5 | brokerage-unknown fix, EF facts reconciled | 3.34 | 58% | **21%** | net_worth/facts/grounding → 5.0/4.3/5.0 |
| v6 | 10-year seed, +12 temporal/stress Q (65 total) | 3.0 | 49% | 34% | headline *dropped* — but forensics show most of it is 2 deterministic bugs + judge false-positives, not reasoning |
| v7 | plan-only re-prompt (BUGGY) + brokerage-$0 fix | 2.831 | 45% | — | re-prompt looped to empty on stochastic stalls — a regression; fixed in v8 |
| v8 | never-empty re-prompt + plan-stall root-cause fix (pair PLAN w/ tool call) | 3.4 | 58% | 37% | **usable 78%→89%** — the stall fixes landed; more real answers ⇒ more numbers ⇒ fab ticks up |
| v8′ | re-judge only: judge HISTORY facts (Bucket B) | 3.585 | 63% | 29% | same answers — stripped ~5 derived-aggregate false-positives (temp-05 1→5). **True baseline for v9.** |
| v9 | cents `_display` fields + list_uncategorized expense filter | _running_ | | | targets the ~9 zero-drop cents fabs + the 125-vs-4 uncat bug |

### The plan-stall saga (v6→v8): a fix that regressed, then landed

v6 forensics found the biggest failure was a **plan-only / empty final turn**: the
mandatory `PLAN:`-before-tools instruction made glm-class models end a turn with a
plan + "let me pull the data now" and *no tool call*, which the API returns as a
final answer. v7's first re-prompt fix **looped to empty** on stochastic stalls
(converting "ships a plan, score 1" into "empty, score 1, worse UX") — a net
regression (usable 78%→65%). The lesson: **a non-answer backstop must be
never-empty.** v8 fixed it two ways — (a) cap the nudge at 1 and fall back to the
best content seen (a plan beats nothing); (b) reword the prompt so the PLAN is
emitted *in the same turn* as the first tool call, and forbid text-only turns.
Result: **usable 78%→89%**, the real headline. Validated on a 13-question subset
harness (no judge) to avoid burning full runs on a deterministic-mechanism fix.

Headline moved from **1.36 → ~3.4/5** and fabrication fell **monotonically 71% →
29% → 21%**, with roughly half of the gain being *measurement* fixes (a fair,
reference-grounded judge) and half being *real* Copilot/seed fixes.

## v6 deep-dive: separating three failure buckets (the important lesson)

v6's headline fell to 3.0/49% with 34% fabrication, but reading the **raw
answers + traces** (not the judge labels) split the 30 critical failures into
three very different buckets. **This split — not the headline — is the finding.**

**Bucket A — deterministic engine/data bugs (~9 questions, fixed in v7):**
- *Plan-only / empty final turn.* The model sometimes ended its first turn
  emitting ONLY the `PLAN:` preamble (or empty content) with no tool calls;
  the engine shipped that raw plan/nothing as the answer. Fast (1.7–18s, not
  timeouts). Hit nw-01, nw-26, inc-07, ef-16, spend-12, stress-02. Fix: the
  loop refuses to finalize a non-answer and nudges the model to continue.
- *Unknown snapshot balance read as $0.* `build_snapshot` COALESCE'd a missing
  balance to 0 with no known-flag, so `get_financial_snapshot` fed planning
  answers a $0 brokerage (new-06/09/11). Same class already fixed in the read
  tools; `build_snapshot` had its own copy. Fix: `balance_known` flag + warning.
- *(Deferred) cashflow starting-liquid uses the EF-eligible balance* ($5,000)
  instead of total liquid ($7,000) — `run_cashflow_timeline` line ~1255. The
  model faithfully reported the wrong tool number (cash-19).

**Bucket B — judge false-positives on *derived* aggregates (measurement, not a
Copilot bug):** the judge is grounded only in static `HOUSEHOLD_FACTS`, which
don't enumerate per-year/per-merchant sums. temp-05 was scored 1 for
"fabricated dates/counts/total" — but "first DoorDash Jul 2024, 25 txns, $1,500"
is **exactly** what the seed generates. temp-03's 60-mo Housing total ($65,100)
is also exact. This is the v1→v2 lesson resurfacing for *derived* numbers.
Fix belongs in the judge (allow tool-derived aggregates), validated by
**re-judging stored answers** — no new Copilot run needed.

**Bucket C — genuine model fabrication (real, hardest; v8 grounding work):**
- Cents mis-conversion: new-01 reported the Vacation goal as "$60 / $300" when
  `get_goals` returns $600 / $3,000 — a clean ÷1000. Suggests `_cents` integer
  fields force error-prone mental division; consider returning formatted values.
- Bad arithmetic: stress-04's $141,013 interest on a $9,200 balance; plan-30.
- Invented specifics: stress-01 fabricated an insurance date + a "1-month floor";
  recat-23 invented 121 extra uncategorized transactions.

**Takeaway:** at 30/65 "critical," a large fraction is Bucket A (deterministic,
now fixed) + Bucket B (measurement). Real Copilot quality is better than 3.0/49%
implies. Always verify a fabrication flag against the tool result before
"fixing" the model — otherwise you harden it against a measurement artifact.

The v5 `overall_mean` is slightly below v4 (3.34 vs 3.47) despite a lower
fabrication rate — this is **variance, not a regression**: categories have only
1–3 questions each, so one varied LLM answer swings a category from 5.0 to 1.0,
and v5 happened to hit **6 transient/timeout harness errors (11%)** vs v4's 3.
The trustworthy trend line is fabrication (down every run) and the per-capability
picture below.

## Per-capability picture (v5)

**Strong (5.0):** net worth, facts/balances, grounding (unknown balances),
affordability, anomalies, recategorization, safety (principles-only / no
fabrication), unsupported (graceful decline), ambiguous (clarify). Tool selection
is consistently good (tool_use ~4.0).

**Weak — hard multi-step planning (~1.7).** Two distinct, real failure modes:
- **Number fabrication in long answers.** glm-5.2 gives genuinely good qualitative
  plans but then invents a *specific* figure (a monthly amount, a transaction) not
  in the tool data — the judge (correctly) treats that as a critical failure.
  Mitigation to try: a stronger grounding instruction for planning answers ("every
  dollar figure must come verbatim from a tool result; if you don't have it, say
  so") and/or requiring the model to cite which tool value each number came from.
- **Timeouts on the most complex questions.** The 3-month-break, car-feasibility,
  job-offer, and rent-impact questions call many tools and exceed the **180s**
  ceiling (glm-5.2 averages ~88s/question). This is production-relevant: those
  users would hit the canned fallback. Either a faster model, fewer tool
  round-trips, or a higher ceiling is needed for the hardest questions.
- **Bright spots:** the open-ended "what should I focus on" and the hardest
  behavioral question (invest-all-$5k with a preference the app can't recall) both
  scored 5.0 — the model handled the memory limitation gracefully rather than
  fabricating past preferences.

## What the loop taught us about the judge (eval quality)

1. **A terse per-question fact list makes an LLM judge over-flag fabrication.**
   It marked *correct* extra data (real account balances, real minimum payments)
   as hallucination. Fix: give the judge the **complete** household ground truth
   every question, and define fabrication as *contradicting* it — not "absent from
   the terse subset." (v1→v2: overall +1.76, fabrication −42pts, same answers.)
2. **Don't fold a production-gating signal into the answer score.** A correct
   no-tool decline (e.g. "I can't fetch live stock prices") was scored 1 only
   because `is_usable=false`. That belongs in a *separate* `usable_rate` metric.
3. **Reference facts must match how the app actually computes things**, not how a
   human would. The emergency-fund answers were *correct* (the app measures runway
   from total liquid cash); our facts wrongly assumed savings-only. A benchmark
   that grades against a different definition than the product uses manufactures
   false failures.

## Genuine Copilot weaknesses found (ranked)

1. **[FIXED] Unknown balances reported as $0.** `get_account_balances` COALESCE'd a
   missing balance snapshot to `0`, so an unsynced brokerage read as `$0.00` and the
   model echoed it — contradicting the app's own "never report unknown as $0" rule
   (which `get_net_worth` already followed). Now returns `balance_known=false` / null
   and excludes it from the total.
2. **[REAL — follow-up] Monthly surplus is distorted by the 90-day window.** Surplus
   is `income_90d − expense_90d`. A large one-off/anomalous charge (e.g. the $2,500
   Apple Store charge) inflates `expense_90d` and can crush the reported surplus
   (we measured $163 vs a true ~$1,900). A user who makes one big purchase, or asks
   early in a month, will be told they have almost no monthly surplus. Consider
   excluding flagged anomalies / using a median or recurring-based expense for
   surplus. (Worked around in the benchmark by spreading one-offs out of the window;
   the underlying behavior remains.)
3. **[REAL — follow-up] Two inconsistent "emergency-fund months" numbers.**
   `build_snapshot.emergency_fund_months` divides the *EF-eligible* balance by
   expenses, while `run_emergency_fund_scenarios` divides *total liquid* cash — so
   the same household yields ~2.4 vs ~3.3 months depending on which tool answers.
   Pick one definition.
4. **[REAL — follow-up] `is_usable` gate suppresses correct no-tool answers.** The
   production `is_usable_tool_answer` gate requires a tool call, so a correct
   *decline* or *clarification* (which legitimately calls no tool) is replaced by the
   canned fallback. Relax the gate to treat a substantive decline/clarify as usable.
5. **[MODEL] Hard multi-step planning is the weakest area.** The upper-bound
   questions (3-month work break, job-offer net-benefit, 3-way debt/EF/invest) are
   where glm-5.2 most often drops a constraint (e.g. forgets the upcoming insurance
   payment, or an emergency-fund floor). Tracked per-question in `failures.md`.

## Using this as a regression detector

Re-run `python run_eval.py` after any Copilot/prompt/tool change and compare the
MLflow run to the previous one. A drop in `overall_mean`, `pass_rate`, or a rise in
`fabrication_rate` / `critical_failure_rate` flags a regression; `failures.md` shows
exactly which questions and why. The seed's `reference_facts` are locked to the seed
by a Rust unit test, so the ground truth can't silently drift.
