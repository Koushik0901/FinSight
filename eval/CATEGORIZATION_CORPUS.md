# Categorization labeled corpus (issue #89) + eval harness (issue #88)

Part of the decomposition scoped on epic #74. This is **not** the Copilot
answer-quality benchmark documented in `eval/README.md` — it's separate
machinery for a separate question: *is the categorizer's precision claim
real?* See the code doc comments in
`crates/finsight-eval/src/categorization/` (`mod.rs` is the entry point) for
the harness internals; this file documents the **data** side.

## Status: synthetic seed only — no real corpus exists yet

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

> SYNTHETIC SEED DATA baseline — computed against invented, clearly-fictional
> transactions … This is NOT a measured real-world precision claim.

Treat every synthetic-seed number as "the harness works," not "the
categorizer is 91% precise."

## Corpus file format

One JSON object per line (JSONL), matching the `eval/benchmark.jsonl`
convention: blank lines and `//`-prefixed comment lines are skipped by the
loader (`crates/finsight-eval/src/categorization/corpus.rs::load_corpus_jsonl`).

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
   reuse an existing `merchant_id` for another transaction from a merchant
   already in the corpus.
2. Run the harness:
   ```bash
   cargo run -p finsight-eval --bin categorization_eval -- \
     --corpus eval/categorization_corpus.synthetic.jsonl
   ```
   Add `--out <path>` to write the JSON report to a file instead of stdout,
   or `--holdout-fraction` / `--seed` to change the split. If you changed the
   corpus, regenerate the committed baseline artifact too (see the command in
   "Recorded baseline" above) so the checked-in numbers don't drift from the
   checked-in corpus.
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
