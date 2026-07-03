# Phase 6 — Final Report

Data-correctness overhaul, driven by diagnostics against the real imported
AMEX + Tangerine data (1,372 transactions), fixing root causes with generic,
tested code. Delivered as 8 reviewed commits on `phase-6-data-correctness`.

## 1. Root causes found
- **Recurring (#9):** detection was `occ ≥ 2 AND gap 5–400 days` — flagged every
  merchant seen twice, so ride-hailing, groceries, dining, transit, and even
  card payments/e-transfers became "subscriptions" (the "18 subscriptions" bug).
- **Charts (#2):** report windows were anchored on wall-clock `now`, but the
  data is historical → the default month/quarter charts were empty despite data.
- **Balance (#1):** AMEX's only balance snapshot is `source='seed', cents=0`;
  already handled — shown as "Balance not set", never a real `$0` (Phase 1‑4).
- **Categorization (#3):** builtin ran on import but the LLM never had a provider
  (fixed by the Phase 5B `.env` bootstrap); a latent categorizer bug aborted the
  whole job on a single LLM-hallucinated `txn_id` (FK violation).
- **Needs Review/Anomaly (#5):** empty because no LLM confidences existed and
  anomaly detection had never run.
- **Categories (#8):** an unlabelled "type" dropdown; no create/rename/archive;
  no per-category rules/guidance.
- **Pagination (#4):** the account view fetched with `limit: null` (all rows).

## 2. Data invariants defined
`docs/phase6-data-invariants.md` — the unifying four-state distinction
(**known / real-zero / unknown / no-data / query-failure**) threaded through
balances, charts, review, anomalies, insights, and Copilot; plus invariants for
accounts, transactions, categories, recurring, insights, and reset/re-import.

## 3. Balances / charts fixes
- Charts anchor windows on the most recent **month with activity**
  (`scope_month_list`), so historical imports populate; default scope "year";
  honest "No transactions in this period" empty state distinct from
  loading/error. Row-level query failures already surface (Phase 3), never
  fabricated `$0`.
- Balance honesty verified already-correct end-to-end (Accounts list, account
  detail with a "Set balance" CTA, Today) — unknown balances show "Balance not
  set" and are excluded from net worth.

## 4. Categorization fixes
- Fixed the categorizer FK abort: validate each LLM result's `txn_id` against
  the batch (mirrors the existing `category_id` guard); a single write failure
  is logged and skipped instead of aborting the job.
- Category `guidance` is fed into the LLM categorizer prompt and the Copilot
  recategorization tool, so both follow the user's per-category intent.
- **Live result on real data:** 439 uncategorized → **0**; 439 LLM
  categorizations; the job now completes cleanly.

## 5. Pagination implementation
`useInfiniteTransactions` (tanstack `useInfiniteQuery`) over the backend's
existing `limit`/`offset`, 50 rows/page, with the active filter in the query key
so sort + filter + search + pagination compose; "Load more" + end-of-list marker.

## 6. Needs Review / Anomaly behaviour
- **Anomaly (#5):** new deterministic `recompute_anomalies` (median + MAD robust
  outlier vs the merchant's own history; excludes transfers; clears stale flags;
  writes a reason). On real data it flags **26** genuine outliers (Air Canada
  $2,287 vs typical $110, etc.). Recomputed on startup and after import. The
  transactions "anomalies" filter (`is_anomaly = 1`) now populates.
- **Needs Review:** `ai_confidence < 0.6` for LLM items. After the live
  categorization it holds **68** real low-confidence items; honestly empty
  before the LLM runs.

## 7. Insights fixes
The subscription insight derives from the recurring detector, so **#9 fixes it
automatically** ("18 subscriptions" → ~8 real ones + correct annual cost).
"What the agent has learned" already draws only from user-approved corrections
(`source='user'` / approved bundles) — the invariant was already met.

## 8. Today page decision
Kept and improved (it already had the right hooks). Surfaces the now-computed
anomalies + needs-review as attention chips (with an honest "nothing needs your
attention" state), and "Looking ahead" falls back to recurring commitments when
nothing is due within two weeks — so it stays useful on historical data.

## 9. Categories / subcategories / rules
- Migration V034 adds `guidance`; repo + commands for create / rename / archive /
  set_guidance; UI for new category, per-row Manage (rename, guidance textarea,
  archive), and a clarified "Spending type" column.
- Merchant-pattern rules already exist (create_rule/toggle_rule); guidance is the
  free-text complement, consumed by categorizer + Copilot.
- Subcategories: the UI does not imply them (flat list within groups) — not added.

## 10. Recurring detection redesign
`finsight-core::recurring` — groups by a shared **normalized merchant**, then
classifies into subscription / bill / income / transfer / repeat-purchase using
cadence regularity, amount stability within a ~15% tolerance band (FX-aware via
vendor hints), minimum occurrences, category exclusions, vendor allowlists, and
transfer/card-payment detection — returning **kind + confidence + reasons**. On
real data: **8 subscriptions** (Spotify, OpenAI, Claude, OpenRouter, Anthropic…)
+ bills (Freedom Mobile), with EVO/Uber Eats/Walmart/Dominos correctly excluded.

## 11. Copilot grounding changes
All Copilot tools are deterministic: `get_net_worth`, `get_spending_breakdown`,
`get_recurring_bills` (via `detect_recurring`), `find_anomalies` (via the new
`is_anomaly`), `search_transactions`. No invented numbers; empty/ambiguous data
is clarified or fails gracefully (Phase 5B).

## 12. Tests added/updated
merchant normalization (4); recurring regression both directions incl. real
vendors (2); reports anchoring (5); anomaly (4); category repo CRUD/guidance (4);
Categories management UI (1); pagination; Today; Recurring; delete→reseed→
recompute cycle (1); categorizer hallucinated-txn_id regression (1). Full Rust
workspace + 278 frontend green, 0 TS errors. Plus `#[ignore]` live harnesses
(`phase6_categorize_live`, `phase6_diagnostics`).

## 13. Manual UI validation results
- Recurring detector on real data: 18 false-positive "subs" → 8 real subs + 2
  bills + transfers/repeat-purchases excluded (diagnostics).
- Live LLM categorization on real data: 439 → 0 uncategorized, 68 needs-review.
- Anomaly detector on real data: 26 genuine outliers with reasons.
- Delete → recompute → re-seed cycle: reproducible test proves every derived
  surface resets and rebuilds.

## 14. Remaining limitations
- The full 6-file **import → Delete-All → re-import** cycle was verified by a
  reproducible test and the real 2-file dataset, not clicked through the desktop
  UI for all 6 CSVs (import UI is per-file and crash-prone). The reset wipes all
  derived tables and the recompute paths are wired + tested.
- The recurring detector can still mis-bucket a rare uncategorized restaurant as
  a bill if it happens to look statistically regular; once categorized (now the
  case after the live run) the category exclusion resolves it.
- "Needs Review" depends on the LLM having run; on a fresh import it populates
  after auto-categorization completes.
