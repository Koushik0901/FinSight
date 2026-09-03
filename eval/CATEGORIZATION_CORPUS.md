# Categorization labeled corpus (issue #89) + eval harness (issue #88)

Part of the decomposition scoped on epic #74. This is **not** the Copilot
answer-quality benchmark documented in `eval/README.md` — it's separate
machinery for a separate question: *is the categorizer's precision claim
real?* See the code doc comments in
`crates/finsight-eval/src/categorization/` (`mod.rs` is the entry point) for
the harness internals; this file documents the **data** side.

## Two corpus files exist. NEITHER is real data.

| File | Rows | Merchants | Purpose |
|---|---|---|---|
| `eval/categorization_corpus.synthetic.jsonl` | 33 | 29 | Hand-curated seed. Every prediction hand-verified against `KEYWORD_MAP`; its baseline (`eval/categorization_baseline.synthetic.json`) is pinned byte-exact by a test. **Not extended by this section — see below.** |
| `eval/categorization_corpus.synthetic_multi_archetype.jsonl` | ~2,750 | ~330 | Generated (`eval/generate_synthetic_corpus.py`), 8 behavioral archetypes, stress-tests the harness at real scale. **Documented in full below.** |

> **Reframing for #89 (2026-09-03, cbe325d):** A checked-in "real" labeled corpus **cannot** be honestly built by an agent — any labels it invented would be fabricated, and the repo owner's real transactions becoming a public artifact would be a privacy incident (see `crates/finsight-eval/src/categorization/private_eval.rs` module docs and `finsight-server/src/admin_eval.rs`). The real ground truth is **per-instance, local-only**: `source='user'` corrections in the calling instance's own SQLCipher DB, measured via `private_eval::run_private_eval` / `centroid_calibration` over a merchant-disjoint holdout. Synthetic corpora remain the checked-in harness smoke-test at scale; private eval is the real precision gate. Issue #89 is therefore reframed as "local-private, not checked-in" — see `eval/CENTROID_BASELINE.md` Slice 5 calibration (`threshold::calibrated_threshold_for_gate` ≥98% & ≥30) which now prints `NONE qualifies` for CA real data.

**Read this twice: NEITHER checked-in file satisfies issue #89's literal ask for REAL checked-in data.** #89's original acceptance (checked-in corpus + volume/diversity stats) is intentionally not met as a public artifact; its *measurement* acceptance is met via the private, per-instance path that never leaves the machine. Both files here are 100% invented. More synthetic rows is still zero checked-in real rows — and that is the honest, privacy-preserving design. The only real corpus lives privately on each self-hosted instance.
## Status: checked-in synthetic + private local real — #89 reframed (2026-09-03)

*Checked-in* real corpus is intentionally **not** built (privacy + honesty — see reframing callout above). The small 33-row hand-curated seed below and the larger multi-archetype corpus are the only checked-in files; the **real** corpus is per-instance `source='user'` corrections measured via `private_eval` (merchant-disjoint holdout, never leaves the machine).

*(This section is about the small, 33-row hand-curated seed. For the much
larger multi-archetype corpus, jump to "The multi-archetype synthetic
corpus" below.)*

**`eval/categorization_corpus.synthetic.jsonl` is entirely invented data.**
Every merchant name is fictional, chosen to exercise the harness end-to-end
(confusion matrix, merchant-disjoint split, threshold sweep, JSON report) —
none of it is a real transaction, and none of it is drawn from any real
user's account data. A handful of entries deliberately reuse merchant
keywords that already ship in `finsight_core::categorize::KEYWORD_MAP`
(generic words like "pizza", "parking", "pharmacy", "mortgage", plus a few
widely-known public brand names like Costco/Netflix/Best Buy) so the *real*
`builtin` matcher has something legitimate to hit — that's testing shipped
code against its own public vocabulary, not fabricating a result.

Any number this harness reports against the synthetic seed carries the
`caveat` field in its JSON output verbatim:

> SYNTHETIC SEED DATA baseline — the corpus file declares `provenance:
> synthetic` … This is NOT a measured real-world precision claim.

Treat every synthetic-seed number as "the harness works," not "the
categorizer is 91% precise."

## Provenance is declared by the corpus, not by the harness

Every corpus file **must** declare where its labels came from, on a comment
line:

```
// provenance: synthetic
```

Accepted values: `synthetic` | `real`. The rules, all enforced by
`crates/finsight-eval/src/categorization/corpus.rs`:

- A corpus that declares **no** provenance **fails to load.** There is no
  default in either direction.
- The report's `caveat` string and its `corpus_provenance` field are both
  *derived* from this directive (`CorpusProvenance::caveat`). Nothing in the
  harness hardcodes synthetic language.

This exists because the caveat used to be a fixed string literal in
`report::run`, which failed the harness's single most important property in
both directions: a real corpus pointed at via `--corpus` would have silently
inherited "SYNTHETIC SEED DATA", and the obvious fix (edit or flag away the
literal) would have silently stripped the warning from the default, no-arg
invocation that reads the synthetic seed. Now a real corpus declares `real`
and gets a caveat with no synthetic language in it, the synthetic seed cannot
lose its warning without an explicit edit to the corpus file, and both
directions are pinned by tests (`corpus.rs`:
`bundled_seed_declares_synthetic_provenance`,
`a_corpus_with_no_provenance_directive_is_rejected`; `report.rs`:
`a_real_corpus_report_does_not_claim_synthetic`).

## `merchant_id` validation

The merchant-disjoint split partitions on **exact string equality** of
`merchant_id`, so the loader rejects two things outright rather than letting
them silently defeat the split:

- **Near-duplicate spellings.** `m-brightloaf`, `M-Brightloaf`, and
  `"m-brightloaf "` are one merchant but three split keys — they can land on
  opposite sides of the holdout while `merchant_sets_disjoint` still reports
  `true`. Any two distinct `merchant_id` values that agree after trim +
  lowercase are a load error naming both spellings. Pick one canonical form.
- **Blank ids.** An empty or whitespace `merchant_id` buckets every such row
  into a single pseudo-merchant. Also a load error.

Normalization is used only to *detect* these; the split still compares the
exact string, so the fix is to correct the data rather than have the harness
paper over it.

## Corpus file format

One JSON object per line (JSONL), matching the `eval/benchmark.jsonl`
convention: blank lines and `//`-prefixed comment lines are skipped by the
loader (`crates/finsight-eval/src/categorization/corpus.rs::load_corpus_jsonl`),
except for the required `// provenance:` directive described above, which the
loader parses before skipping comments.

```json
{"id": "g1", "merchant_id": "m-brightloaf", "merchant_text": "Brightloaf Grocers #12", "category": "groceries", "notes": "optional"}
```

| Field           | Required | Meaning |
|-----------------|----------|---------|
| `id`            | yes      | Free-form row id for traceability. Not used by the split or any matcher. |
| `merchant_text` | yes      | Raw description text as a categorizer would see it (the `transactions.merchant_raw` equivalent). This is what `builtin`/`rule`/`llm` predictors match against. |
| `merchant_id`   | yes      | **Normalized merchant identity — the field the merchant-disjoint split partitions on.** Multiple `merchant_text` variants of the same real-world merchant (different store numbers, punctuation, "renewal" suffixes) must share one `merchant_id`, or the split can't group them and the "no merchant leaks across halves" guarantee is meaningless for that merchant. |
| `category`      | yes      | Ground-truth category id. Uses the same ids as `finsight_core::categorize`'s starter categories (`groceries`, `dining`, `transport`, `shopping`, `travel`, `gifts`, `housing`, `utilities`, `subscriptions`, `health`) so `builtin`/`rule` predictions are directly comparable without a translation layer. A real corpus with custom user categories would need to either map onto these or the harness's category-comparison would need to widen — not needed yet at synthetic-seed scale. |
| `notes`         | no       | Free text on why the row was included (e.g. a deliberate "trap" case, or "no keyword overlap by design"). |

## Recorded baseline (synthetic seed — NOT a real precision claim)

Committed artifact: **`eval/categorization_baseline.synthetic.json`**, regenerated with

```bash
cargo run -p finsight-eval --bin categorization_eval -- \
  --corpus eval/categorization_corpus.synthetic.jsonl \
  --out eval/categorization_baseline.synthetic.json
```

**Corpus volume + merchant diversity** (issue #89's "document volume and
merchant-diversity stats" criterion):

| Metric | Value |
|---|---|
| Total labeled examples | 33 |
| Unique merchants | 29 |
| Merchants with >1 example | 4 (`m-brightloaf`, `m-cloudnote`, `m-northwave-internet`, `m-pinecrest-pizza` — 2 each) |
| Categories represented | 10 |

Category distribution: dining 6, groceries 6, shopping 4, health 3,
subscriptions 3, transport 3, utilities 3, housing 2, travel 2, gifts 1.

**Baseline precision/coverage** at split `holdout_fraction=0.3`, `seed=42`
(holdout = 11 examples across 9 merchants):

| Source | Scope | Predicted / Total | Correct | Precision | Coverage |
|---|---|---|---|---|---|
| `builtin` | full corpus | 16 / 33 | 15 | **93.8%** | 48.5% |
| `builtin` | merchant-disjoint holdout | 4 / 11 | 4 | 100% | 36.4% |
| `rule` | full corpus | 8 / 33 | 8 | **100%** | 24.2% |
| `rule` | merchant-disjoint holdout | 5 / 11 | 5 | 100% | 45.5% |

Reading these honestly:
- `builtin`'s single full-corpus error is the deliberate `Esso Corner Store`
  trap row (gas-station keyword → `transport`, ground truth `groceries`).
  That one error costs 6.2 points of precision at n=16, which is itself a
  demonstration of why #89's real-corpus volume guidance matters — at this
  sample size, precision is enormously sensitive to a single row.
- `builtin` covers under half the corpus because most merchant names in the
  seed are fictional with no `KEYWORD_MAP` overlap by design — that's an
  honest coverage gap, not a bug.
- `rule`'s 100% precision is **circular and carries no information**: the
  synthetic rules in `predictors::synthetic_rules` were authored to target
  specific corpus rows correctly. It demonstrates the rule pass is wired into
  the harness, nothing about real-world rule precision.
- The holdout numbers come from 4 and 5 predictions respectively. At that
  size a confidence interval is meaninglessly wide — these are smoke-test
  numbers proving the split feeds the matrix, not measurements.

Every one of these numbers is computed against invented data. None of them
supports or refutes the epic's "≥98% precision" claim.

The committed artifact is **pinned by a test**
(`report.rs::committed_baseline_artifact_matches_the_bundled_corpus`): it must
equal a fresh run over the bundled corpus at `holdout_fraction=0.3`,
`seed=42`. Editing the corpus or the harness without regenerating the artifact
fails `cargo test -p finsight-eval` instead of leaving two checked-in files
that quietly disagree.

Each source in the JSON carries **two** threshold sweeps, scoped by field
name: `threshold_sweep_full_corpus` (in-sample, over the whole corpus) and
`threshold_sweep_holdout` (over the merchant-disjoint holdout only). Both are
degenerate single steps today because `builtin`/`rule` always predict at
confidence 1.0 — but once a confidence-bearing source exists (#90's encoder),
**an auto-apply cutoff must be picked from the holdout curve.** The in-sample
curve overstates precision, which is exactly the direction that would break a
≥98% gate.

## The multi-archetype synthetic corpus

`eval/categorization_corpus.synthetic_multi_archetype.jsonl`, generated by
`eval/generate_synthetic_corpus.py` (stdlib-only Python, no new dependency).
This extends the labeled-corpus work from issue #89 ("Slice 2b") **without
claiming to close it**. Read the callout at the top of this document again
if you skipped it: **this file is entirely invented data and does not
satisfy #89's real-data requirement, no matter how large it is.** Every
number the eval harness computes from it is a harness smoke-test at
realistic scale, not a real-world precision claim — see
`CorpusProvenance::caveat` and the `# provenance: synthetic` directive at
the top of the file itself.

### Why this exists alongside the small seed

The 33-row hand-curated seed is deliberately tiny (small enough that every
prediction could be hand-verified against `KEYWORD_MAP` — see its pinned
baseline). That's the right design for a seed, but it's too small to
meaningfully stress-test two things that only show real behavior at scale:
`validate_corpus`'s near-duplicate-`merchant_id` detection (trivial to pass
by accident with 29 merchants; harder with ~330), and the merchant-disjoint
split's disjointness guarantee across categories with very different
merchant-pool sizes. This corpus is ~330 fictional merchants across ~2,750
rows and 8 distinct behavioral archetypes, specifically to exercise the
harness at a scale closer to what a real corpus (per the "what a real
corpus would need" section above) would eventually need to look like.

### Methodology: multi-archetype behavioral simulation

Rather than one flat merchant list, the generator builds **8 distinct
fictional archetypes** — generalized personas, never a specific real
person or household (per the standing project directive against baking any
one real user's data into a design target: `student_urban`,
`young_professional_urban`, `dual_income_family`, `remote_gig_worker`,
`retiree`, `frequent_traveler_high_income`, `rural_household`,
`new_homeowner`. Each archetype declares category weights, recurring
categories (housing/utilities/subscriptions merchants that repeat monthly
with the same `merchant_id`), and seasonal modifiers (e.g. gift-giving
spikes in Nov/Dec, a winter travel spike for the retiree archetype, a
summer travel spike for most others), then a 9-month window (Nov 2025 → Jul
2026) is simulated per archetype. See the full design rationale in
`eval/generate_synthetic_corpus.py`'s module docstring and the
`ARCHETYPES` dict — it's heavily commented, treat it as the source of
truth over this summary.

**Methodology citation, not a data source.** The archetype-weighted
category/amount-distribution approach is inspired by
[`namebrandon/Sparkov_Data_Generation`](https://github.com/namebrandon/Sparkov_Data_Generation)
(MIT licensed), a synthetic transaction generator that assigns simulated
customers a demographic profile with weighted category/amount
distributions. We cite it for *how to structure* a multi-persona
generator. We do **not** import its data, its Faker-generated generic
merchant names, or its 16-category taxonomy (it doesn't map onto
FinSight's 10 starter categories). Every merchant name, archetype
definition, and word bank in this corpus is this script's own invention.

### Merchant catalog

~330 unique fictional merchant identities across the 10 starter categories,
each with 8-9 realistic surface-text variants generated per merchant
(9 when the base name contains `&` or an apostrophe and so gets an extra
punctuation variant, 8 otherwise) sharing one `merchant_id` — not every
generated variant necessarily appears in a final row, since which variant a
given transaction uses is itself a random draw, but the full 8-9-variant
pool exists for every merchant (store numbers, `POS PURCHASE` tags,
city+province suffixes, trailing reference digits, ALL-CAPS vs Title Case,
`&`/`AND` and apostrophe-drop punctuation variants — standard,
publicly-known bank/card statement formatting conventions, not derived
from any specific account). Every name
is invented; **no real brand names appear anywhere in this file**, unlike
the small seed, which deliberately uses a handful of real public brand
names (Costco, Netflix, Best Buy) for specific rows. This corpus took the
opposite, stricter approach — see "Declared keyword-hit ratio" below for
why that constrains which categories can have any keyword-hit merchants at
all.

Per-category merchant-catalog sizes are **not uniform**: recurring-only
categories (housing, utilities, subscriptions) are deliberately smaller
(10/20/28 merchants respectively), because their realistic diversity
ceiling is structurally low — a household has exactly one landlord or
mortgage lender, not a dozen, so at most ~6-14 distinct merchants can ever
appear across 8 archetypes regardless of how large the pool is. Sizing
those pools like the high-frequency variable categories (dining: 55,
groceries: 48) would just leave dozens of invented merchants that never
appear in a single row.

### Declared keyword-hit ratio — and why several categories are 0%

Per the task design: we do **not** let invented merchant names organically
end up containing `finsight_core::categorize::KEYWORD_MAP` substrings by
chance of word choice — that would make measured `builtin` precision a
property of naming taste, not of the keyword table (the exact circularity
already flagged for the small seed's `rule`-source 100% precision, quoted
above). Instead every merchant is generated into an explicit "hit" or
"no-hit" bucket, verified against a Python mirror of the real `KEYWORD_MAP`
(same order — first match wins), and every row's `notes` field says which
bucket its merchant belongs to (`"keyword-hit by design (matches
KEYWORD_MAP '...')"` or `"no builtin keyword overlap by design — coverage
gap"`).

Declared per-category targets (checked by
`crates/finsight-eval/src/categorization/multi_archetype.rs
::keyword_hit_ratio_matches_declared_target_per_category`, tolerance ±10
percentage points):

| Category | Declared hit ratio | Why |
|---|---|---|
| dining | 35% | Generic words exist: pizza, burger, sushi, bakery, cafe, coffee, donut, restaurant |
| groceries | **0%** (structural) | Every real KEYWORD_MAP groceries entry is a specific real chain brand name (Walmart, Costco, Safeway, ...) — this corpus's "never real brand names" rule forbids reproducing any of them |
| transport | 20% | Generic words: parking, transit |
| shopping | **0%** (structural) | Every real KEYWORD_MAP shopping entry is a specific real retailer brand name (Amazon, Best Buy, IKEA, ...) |
| travel | 10% | Only one non-brand keyword exists: "auberge" (generic French noun for "inn") |
| gifts | **0%** (structural) | The real KEYWORD_MAP has literally zero entries mapping to "gifts" — already true of the small seed too |
| housing | 55% | Generic phrases: mortgage, property mgmt/management, " rent " |
| utilities | 25% | One generic word: "hydro" |
| subscriptions | 15% | One generic phrase: "membership fee" |
| health | 40% | Generic words: pharmacy, dental, clinic, physio |

**Three categories (groceries, shopping, gifts) are declared 0% not by
choice but by structural necessity** — their real `KEYWORD_MAP` entries are
(almost) exclusively specific brand names, which this corpus's fictional-
only policy forbids reproducing. This is itself a finding worth surfacing:
`builtin`'s real-world coverage for those three categories, on genuinely
fictional data, is mechanically bounded at (near) zero — any coverage
`builtin` gets on them in production comes entirely from matching a real
brand name in a real transaction, which no synthetic corpus with this
naming policy can exercise.

**Measured ratio at time of writing** (merchant-level; row-level numbers
have more variance for recurring-dominated categories since only a handful
of distinct merchants ever get used as anyone's monthly bill — see
`multi_archetype.rs` for why the tolerance is 10pp, not tighter):

| Category | Declared | Measured (merchant-level) |
|---|---|---|
| dining | 35% | 34.5% |
| groceries | 0% | 0.0% |
| transport | 20% | 19.0% |
| shopping | 0% | 0.0% |
| travel | 10% | 9.4% |
| gifts | 0% | 0.0% |
| housing | 55% | 60.0% |
| utilities | 25% | 25.0% |
| subscriptions | 15% | 14.3% |
| health | 40% | 40.6% |

### Explicit exclusions

**Zero transfer-shaped rows.** No `E-TRANSFER`, `INTERNET TRANSFER`,
`PAYMENT THANK YOU`, `AUTOPAY`, or any vocabulary that would trip
`finsight_core::categorize::is_transfer` — transfers are never categorized
by this product at all, so including them as labeled category ground
truth would misrepresent what this corpus tests. **Zero investment/
brokerage-shaped rows** — FinSight routes those through `activity_type`, a
DB column this flat corpus format has no field for. Both are enforced
twice: at generation time (the Python generator asserts on every emitted
row) and again by
`multi_archetype.rs::zero_transfer_or_investment_vocabulary_present`, which
greps the committed file for the same vocabulary AND calls the real
`finsight_core::categorize::is_transfer` directly. Neither list is
speculative — do not "fix" a future gap by adding transfer/investment rows
here; that's out of scope by design, not an oversight.

### Expected near-zero `rule` coverage

`predictors::synthetic_rules()` (`crates/finsight-eval/src/categorization/
predictors.rs`) is a hand-picked list of 6 patterns authored specifically to
match the SMALL seed's merchant names (`brightloaf`, `cloudnote`,
`craftbox`, `swiftcab`, `riverbend diner`, `thoughtful gifts`). This
corpus uses an entirely different, disjoint fictional-merchant vocabulary,
so `rule` coverage on it collapses to (essentially) zero — **this is
expected, not a regression.** Measured at time of writing: `rule` full-
corpus coverage = 0.00, precision = n/a (zero predictions made).

We deliberately did **not** add new patterns to `synthetic_rules()`
targeting this corpus's merchants. Two reasons: (1) that list is shared
code, and `report.rs`'s tests (`report_has_builtin_and_rule_sources_with_
sane_numbers`, and the byte-exact pin
`committed_baseline_artifact_matches_the_bundled_corpus`) assert specific
numbers against the SMALL seed — adding rules would change `rule`
predictions on the small seed too, silently invalidating the pinned
baseline this task was explicitly told not to touch; and (2) it would
reintroduce the exact circularity this whole section exists to avoid: a
rule authored to match a specific corpus's specific merchant names proves
the rule pass is wired up, not that rule-based categorization generalizes.
An earlier draft of this generator's invented word list accidentally
reused ~10 words from the small seed (`Pinecrest`, `Brightloaf`,
`Riverbend`, ...), which nudged `rule` coverage to a nonzero-but-
near-meaningless 1% at 29% precision purely by coincidental substring
collision — a small real demonstration of exactly this risk. That overlap
was removed; the two corpora's merchant vocabularies are now fully
disjoint by construction.

### Scale, runtime, and final numbers (measured, not pinned)

Generated with **fixed seed `20260725`** (`eval/generate_synthetic_corpus.py`'s
`SEED` constant) — regenerating with an unchanged seed and script reproduces
the file byte-for-byte (verified empirically before committing).

| Metric | Value |
|---|---|
| Total labeled rows | 2,751 |
| Unique merchants actually used | 330 |
| Merchant catalog built (some unused, esp. in low-frequency categories) | 347 |
| Archetypes | 8 |
| Simulated window | 9 months (Nov 2025 – Jul 2026) |
| Categories represented | 10 / 10 |

Per-category row counts (deliberately **unbalanced**, matching real-world
transaction frequency — dining/groceries/transport are naturally much
denser than travel/gifts, and this corpus does not force artificial
balance):

| Category | Rows |
|---|---|
| dining | 599 |
| transport | 594 |
| groceries | 487 |
| shopping | 286 |
| subscriptions | 225 |
| gifts | 157 |
| utilities | 127 |
| health | 128 |
| travel | 94 |
| housing | 54 |

Per-archetype row counts range from ~244 (`retiree`) to ~521
(`frequent_traveler_high_income`), reflecting each persona's realistic
transaction frequency (a frequent traveler with a high-dining/high-shopping
lifestyle naturally generates more line items than a modest-spending
retiree over the same 9-month window).

**Baseline precision/coverage** (full corpus, `holdout_fraction=0.3`,
`seed=42` — same split parameters as the small seed's pinned baseline, for
comparability; this one is **not pinned**, see "What NOT to do" below):

| Source | Scope | Coverage | Precision |
|---|---|---|---|
| `builtin` | full corpus | 17% | 100%* |
| `builtin` | merchant-disjoint holdout | 17% | 100%* |
| `rule` | full corpus | 0% | n/a (zero predictions) |
| `rule` | merchant-disjoint holdout | 0% | n/a |

**\*Read this precision number correctly — it is 100% by construction, not
a strong result.** Unlike the small seed (which deliberately includes one
"trap" row, `Esso Corner Store`, where a keyword hit is WRONG), this corpus
contains zero deliberate keyword-mismatch traps: every "hit"-bucket
merchant's ground-truth category is, by construction, the exact category
its embedded keyword maps to. `predict_builtin` abstains on every
"no-hit"-bucket merchant (no keyword match), so 100% of it predictions come
from rows that are correct by design. This 100% says nothing about
production's real-world accuracy — it says the corpus's hit-bucket
construction is internally consistent, which is a much weaker (but still
useful, for stress-testing the harness's plumbing) claim.

**Measured pipeline runtime** (load + `validate_corpus` + merchant-disjoint
split + confusion matrix + threshold sweep, both sources, both scopes, over
the full 2,751-row / 330-merchant corpus): **~495ms**, measured by
`multi_archetype.rs::full_pipeline_runtime_is_fast` on the same machine this
corpus was generated on. Comfortably fast — this is pure in-memory text
processing, nothing like the multi-minute SQLCipher-heavy tests elsewhere in
this repo's suite.

### How to regenerate or expand

```bash
python eval/generate_synthetic_corpus.py
```

Regenerating with an unchanged script and `SEED` reproduces the file
byte-for-byte — it's a checked-in static artifact, not something `cargo
test` regenerates. To expand it: adjust `CATEGORY_CONFIG` (per-category
merchant-pool sizes / hit ratios), `ARCHETYPES` (add a 9th persona, or
adjust an existing one's category weights / seasonal modifiers), or
`CATEGORY_FREQUENCY_BOOST` (row-volume tuning), then re-run and re-check the
stats the script prints against this document's tables (update them if
they moved — they are NOT pinned by a test, unlike the small seed's exact
baseline, so nothing forces this doc to be regenerated automatically;
that's a deliberate tradeoff, not an oversight — see "What NOT to do"
below).

After any change, re-run `cargo test -p finsight-eval` — the invariant
tests in `crates/finsight-eval/src/categorization/multi_archetype.rs` will
catch a corpus that no longer meets the scale floor, drops a starter
category to zero rows, drifts the keyword-hit ratio outside tolerance, or
(re-)introduces transfer/investment vocabulary.

### What NOT to do with this corpus

- **Do not pin an exact-equality baseline JSON for this corpus**, unlike
  the small seed's `eval/categorization_baseline.synthetic.json` (which is
  defensible only because a human hand-verified all 16 predictions against
  a 33-row file). A byte-exact pin on 2,700+ rows would be an unauditable
  hash: it would catch drift, but it would also fire on every legitimate
  future expansion of this corpus, training whoever touches it to blindly
  regenerate the pin without reviewing what changed. The invariant tests in
  `multi_archetype.rs` are the intended guardrail instead.
- **Do not treat this as satisfying issue #89.** #89 stays open. This is
  the honest, explicitly-synthetic half of #89's scope — much wider
  stress-testing of the harness — not the real-data half. Real labeled
  data acquisition is unstarted; see "What a real corpus would need" above,
  which applies unchanged to this corpus too.
- **Do not add patterns to `synthetic_rules()` targeting this corpus's
  merchants** (see "Expected near-zero `rule` coverage" above for why).
- **Do not modify `eval/categorization_corpus.synthetic.jsonl` or
  `eval/categorization_baseline.synthetic.json`** when expanding this
  corpus — they are a separate, independently-pinned artifact.

## Fidelity to production: what the `builtin` predictor does and does not model

`predictors::predict_builtin` wraps the real shipped keyword table
(`finsight_core::categorize::builtin_category`), not the full
`apply_builtin_categorization` pass. **Every gap between the two inflates
measured precision — none of them deflates it.** The mechanism is always the
same: production has a gate that makes it emit *nothing* for a row, this
harness has no such gate and emits a prediction, and if that prediction
happens to match the label the harness banks a point production never earned.
For a "≥98% precision" auto-apply gate, inflation is the direction that
matters.

**Modeled (as of this harness):** the `is_transfer` half of production's
transfer skip. `predict_builtin` abstains on any descriptor for which
`finsight_core::categorize::is_transfer` is true, because production does
`if treat_as_transfer { continue; }` and never records a categorization for
such a row. Without the guard, a descriptor like
`AUTOPAY THANK YOU / NETFLIX.COM` (transfer keyword `autopay` **and**
`KEYWORD_MAP` hit `netflix` → `subscriptions`) would score as a correct
builtin prediction against a defensible `subscriptions` label while production
categorizes it as nothing at all. Pinned by
`predictors.rs::builtin_abstains_on_transfer_shaped_rows_that_hit_the_keyword_map`.

**Still unmodeled, all inflating:**

| Production gate | Why it's not modeled | Effect on this harness's `builtin` number |
|---|---|---|
| `transfer_peer_id` pairing (`pair_transfers`) | Needs a matching counter-leg in another account — DB state a flat labeled corpus doesn't carry | Rows production skips as paired transfers get scored here |
| `TransferContext::is_self_transfer` (owner names, owned-bank aliases) | Needs the user's own identity | Same: person-to-own-account moves get scored here |
| Category-existence gate (`existing.contains(cat)`) | Needs the user's `categories` table; a user who deleted a starter category gets nothing where this harness scores a hit | Inflates coverage and precision |
| `activity_category` investment typing (beats the keyword map in production) | **Not modelable at all today** — `LabeledExample` has no `activity_type` field, so the corpus format would have to grow one first | An investment row would be scored on its merchant keyword instead of its activity type |

A real corpus containing e-transfer / card-payment / brokerage descriptors
needs these modeled (or the corpus format extended) before this harness's
`builtin` precision can be quoted as what the shipped pass delivers.

## How the merchant-disjoint split works

`crates/finsight-eval/src/categorization/split.rs::merchant_disjoint_split`
partitions the corpus into a `(reference, holdout)` pair such that **no
`merchant_id` appears in both halves** — enforced by a dedicated test
(`split_is_merchant_disjoint_on_synthetic_corpus`) and verified by a second
test that hand-constructs a leaking pair to prove the disjointness checker
(`merchant_sets_disjoint`) actually catches it
(`disjointness_checker_catches_a_deliberately_leaked_merchant`) rather than
vacuously passing. The split is deterministic for a given seed (same corpus +
seed → same split), so a recorded baseline is reproducible.

This matters because splitting by *transaction* instead of by *merchant*
leaks: a categorizer that has effectively memorized "Brightloaf Grocers" from
one transaction would trivially "generalize" to a second transaction from the
exact same merchant, which proves nothing about a merchant the categorizer
has never encountered. The whole point of the merchant-disjoint holdout is to
measure the thing the epic's "≥98% precision" claim is actually about:
generalization to unseen merchants.

## How to add new labeled examples and re-run

1. Append one JSON line to `eval/categorization_corpus.synthetic.jsonl` (or a
   new corpus file, if/when a real one exists — see below), following the
   field table above. Give each genuinely-new merchant its own `merchant_id`;
   reuse the *exact* existing `merchant_id` for another transaction from a
   merchant already in the corpus (a near-duplicate spelling is a load error,
   by design). **A brand-new corpus file must start with its own
   `// provenance: synthetic` or `// provenance: real` directive** — it won't
   load otherwise, and that directive is what decides the caveat printed on
   every number derived from it.
2. Run the harness:
   ```bash
   cargo run -p finsight-eval --bin categorization_eval -- \
     --corpus eval/categorization_corpus.synthetic.jsonl
   ```
   Add `--out <path>` to write the JSON report to a file instead of stdout,
   or `--holdout-fraction` / `--seed` to change the split. If you changed the
   corpus, regenerate the committed baseline artifact too (see the command in
   "Recorded baseline" above) so the checked-in numbers don't drift from the
   checked-in corpus — the pin test will fail until you do.
3. Run the harness's own tests after any change to the split/confusion/
   threshold logic:
   ```bash
   cargo test -p finsight-eval
   ```
   (`corpus.rs` has a test that loads the bundled synthetic file directly and
   asserts minimum size/diversity, so a malformed edit fails fast.)

## What a real corpus would need (guidance for whoever picks up #89's real data-acquisition work)

This session deliberately did **not** attempt to source or fabricate real
labeled transactions — see the standing project directive against baking any
one real user's data into a design target, and the parent epic's explicit
warning against claiming a precision number without real backing. The
following is scoping guidance for the follow-up work, not something built
here.

**Volume.** At n≈200 auto-apply decisions, a single misclassification moves
measured precision by roughly 0.5 percentage points. A defensible "≥98%,
merchant-disjoint" claim — one with a confidence interval tight enough to
actually distinguish 98% from, say, 95% — needs **order-thousands of
labels**, not hundreds, and needs them spread across enough *distinct
merchants* that the merchant-disjoint holdout itself has meaningful size (a
holdout built from only 20 merchants can't say much about "unseen merchants"
in general, no matter how many transactions per merchant it contains).

**Diversity requirements.**
- Broad merchant coverage — not concentrated in a handful of recurring
  merchants (rent, payroll, one grocery chain). A corpus dominated by a few
  high-frequency merchants overstates precision on the *tail* of merchants a
  real user's ledger actually contains.
- Spread across all category types the app ships (see the table above), not
  just the easy, high-signal ones (subscriptions, common grocery chains).
- Multiple raw-description *styles* per merchant where possible — different
  banks format the same merchant differently (e.g. `STARBUCKS #4821` vs.
  `SQ *STARBUCKS COFFEE`), and a corpus that only ever sees one bank's
  formatting won't reveal formatting-sensitivity bugs.
- Genuinely ambiguous/edge-case rows (see the `Esso Corner Store` trap row in
  the synthetic seed) — a corpus with only "easy" examples inflates measured
  precision relative to what a real ledger will produce.

**Where real labels might plausibly come from.** The most promising source
is a **review UI capturing user corrections over time** — once #94 ("Slice
6") ships a review/correction surface, every `source='user'` correction
recorded via the existing `categorizations` audit trail (see
`crates/finsight-core/migrations/V003__phase3_schema.sql`) is a real labeled
example with real ground truth, accumulating naturally as the app is used.
This is also the *only* signal issue #93 (personalization/calibration) is
scoped to learn from — `builtin`/`rule`/`llm` self-generated labels
explicitly must not be treated as ground truth for training, only for
measuring precision against something else's ground truth. Until that
surface exists (or some other real-label source is identified), #89 has no
path to real data, and #88's numbers stay synthetic-only.

**Methodology to document once real data exists:** where each label came
from (which review surface, which user action), how "ground truth" was
defined for ambiguous cases, and any known biases in how labels accumulate
(e.g. corrections are necessarily biased toward transactions the deterministic
passes got *wrong* or left uncategorized, which is not a representative
sample of all transactions — that skew needs to be accounted for, not
ignored, when computing an overall precision number from correction data
alone).
