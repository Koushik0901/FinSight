//! Tests for the wide, multi-archetype SYNTHETIC corpus
//! (`eval/categorization_corpus.synthetic_multi_archetype.jsonl`), generated
//! by `eval/generate_synthetic_corpus.py`. This extends the labeled-corpus
//! work from issue #89 ("[Slice 2b]") WITHOUT closing it — #89's acceptance
//! criteria require REAL labeled data, and every row in this file is
//! invented. See `eval/CATEGORIZATION_CORPUS.md` for the full methodology
//! (the Sparkov_Data_Generation citation, the archetype design, the
//! declared per-category keyword-hit ratios, and — repeated there more than
//! once, deliberately — an explicit statement that this remains synthetic
//! and does not satisfy #89.
//!
//! This module is a separate file (not appended to `corpus.rs`) so the
//! large-corpus-scale assertions here don't muddy that module's focused
//! format tests, per the existing crate's one-module-per-concern layout.
//!
//! What these tests actually check, in one place:
//! - The corpus **loads without error at this scale** — the real stress
//!   test of `validate_corpus`'s near-duplicate-`merchant_id` detection and
//!   blank-id rejection, which the original 33-row seed is too small to
//!   meaningfully exercise (this file has ~330 distinct merchants).
//! - Volume/diversity floors, full category coverage, and category-id
//!   validity (issue #89's "document volume and diversity" criterion, at a
//!   scale that actually says something).
//! - Zero transfer- or investment-shaped vocabulary (the two exclusions the
//!   task's brief requires, checked against the corpus file directly).
//! - The declared per-category keyword-hit ratio (see
//!   `eval/generate_synthetic_corpus.py`'s `CATEGORY_CONFIG`) is a CHECKED
//!   claim: this calls the REAL `finsight_core::categorize::builtin_category`
//!   (not the Python generator's mirror) against every distinct merchant in
//!   the corpus and asserts the measured hit rate lands within tolerance of
//!   what the generator declared it would be.
//! - The merchant-disjoint split (`super::split`) holds at this scale — the
//!   real stress test the small seed can't provide.
//! - The full harness (`super::report::run`) executes over this corpus and
//!   produces internally-consistent, non-degenerate numbers. Deliberately
//!   **not pinned** (unlike the small seed's
//!   `eval/categorization_baseline.synthetic.json`): a byte-exact pin on
//!   2000+ rows is an unauditable hash that would fire on every legitimate
//!   future expansion of this corpus.

use super::corpus::{corpus_stats, load_corpus_jsonl, CorpusProvenance, LoadedCorpus};
use super::report;
use super::split::{merchant_disjoint_split, merchant_sets_disjoint};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

/// The 10 starter category ids (`finsight_core::categorize::
/// DEFAULT_CATEGORIES`). Not re-exported by that crate as a public list, so
/// mirrored here literally — same convention the existing
/// `eval/CATEGORIZATION_CORPUS.md` field table and the small seed's tests
/// already use.
const STARTER_CATEGORIES: &[&str] = &[
    "dining", "groceries", "transport", "shopping", "travel", "gifts", "housing", "utilities",
    "subscriptions", "health",
];

/// Declared per-category keyword-hit ratio, mirroring
/// `eval/generate_synthetic_corpus.py`'s `CATEGORY_CONFIG[cat]
/// ["declared_hit_ratio"]`. If that Python dict changes, update this list —
/// `keyword_hit_ratio_matches_declared_target_per_category` (below) is what
/// catches drift between the two, since it calls the REAL
/// `finsight_core::categorize::builtin_category`, not the Python mirror.
const DECLARED_HIT_RATIO: &[(&str, f64)] = &[
    ("dining", 0.35),
    ("groceries", 0.0),
    ("transport", 0.20),
    ("shopping", 0.0),
    ("travel", 0.10),
    ("gifts", 0.0),
    ("housing", 0.55),
    // 0.25 -> 1.00 and 0.40 -> 1.00 when the shipped keyword table was
    // expanded for Canadian and US national chains (see KEYWORD_MAP's two
    // regional blocks). These figures describe the SHIPPED TABLE's reach, not
    // a property of the corpus, so improving the table is expected to move
    // them — the test still catches unintended drift, it just now expects the
    // table to actually cover utilities and health.
    ("utilities", 1.00),
    ("subscriptions", 0.15),
    ("health", 1.00),
];

/// Generous but meaningful tolerance for the keyword-hit-ratio check.
/// Housing/utilities/subscriptions are recurring-dominated categories with
/// comparatively few DISTINCT merchants actually used in the corpus (a
/// household has one landlord, not dozens), so their realized ratio has
/// more sampling noise than a high-volume category like dining. 10
/// percentage points is wide enough to absorb that noise while still
/// failing on a real regression (e.g. the ratio silently dropping to near
/// 0%, or a category's hit rows accidentally doubling).
const HIT_RATIO_TOLERANCE: f64 = 0.10;

/// Vocabulary this corpus must never contain, per the task's explicit
/// exclusions. A superset check (mirrors `finsight_core::categorize`'s
/// transfer keyword lists plus an investment/brokerage list that has no
/// real FinSight predicate to call — this corpus's own invented-data
/// exclusion, not a shipped feature).
// Note for future contributors adding merchants to the generator: several
// entries below are bare substrings on purpose (matching production's own
// broad "pairing hint" vocabulary), which means an otherwise-innocent
// invented name containing that substring will trip this check — e.g.
// "wire" below forbids any merchant name containing "wire" at all
// (a "Wiremill Hardware" or "Hardwire Supply Co" would fail). That is
// intentional, not a bug: it mirrors `PAIRING_HINT_KEYWORDS` faithfully.
// If a future merchant name collides, rename the merchant instead of
// narrowing this list.
const FORBIDDEN_VOCAB: &[&str] = &[
    // Transfer vocabulary (finsight_core::categorize::
    // UNILATERAL_TRANSFER_KEYWORDS / PAIRING_HINT_KEYWORDS / CC_COUNTERPARTY_HINTS).
    "payment received - thank you",
    "payment - thank you",
    "payment thank you",
    "paiement merci",
    "autopay",
    "internet withdrawal to",
    "internet deposit from",
    "transfer to account",
    "transfer from account",
    "internal transfer",
    "online banking transfer",
    "tfr-to",
    "tfr-from",
    "transfer",
    "e-transfer",
    "e transfer",
    "email money transfer",
    "electronic funds transfer",
    "eft",
    "preauthorized debit",
    "pre-authorized debit",
    "preauthorized payment",
    "fulfill request",
    "withdrawal to",
    "deposit from",
    "bill payment",
    "money transfer",
    "wire",
    "bill pay",
    "billpay",
    "credit card",
    "card payment",
    "amex",
    "american express",
    "visa",
    "mastercard",
    "master card",
    "capital one",
    "mbna",
    // Investment/brokerage vocabulary (this corpus's own exclusion list).
    "dividend",
    "brokerage",
    "securities",
    "stock trade",
    "mutual fund",
    "etf purchase",
    "capital gains",
    "drip enrollment",
    "margin call",
    "short sale",
    "trade confirmation",
];

fn repo_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

fn corpus_path() -> PathBuf {
    repo_file("eval/categorization_corpus.synthetic_multi_archetype.jsonl")
}

fn bundled() -> LoadedCorpus {
    load_corpus_jsonl(corpus_path())
        .expect("the multi-archetype synthetic corpus must parse and pass validate_corpus")
}

/// The real stress test: this corpus has ~330 distinct merchants across
/// ~2700 rows, several hundred times the small seed's scale. A successful
/// load here proves `validate_corpus`'s near-duplicate-`merchant_id`
/// detection and blank-id rejection hold up at real volume, not just on a
/// handful of hand-picked ids.
#[test]
fn loads_without_error_at_scale() {
    let loaded = bundled();
    assert!(loaded.examples.len() > 1000, "expected thousands of rows, got {}", loaded.examples.len());
}

#[test]
fn declares_synthetic_provenance() {
    let loaded = bundled();
    assert_eq!(loaded.provenance, CorpusProvenance::Synthetic);
}

/// Issue #89's "document volume and merchant-diversity stats" acceptance
/// criterion, checked against a floor rather than pinned — this corpus is
/// expected to grow over time without breaking this test.
#[test]
fn row_count_and_merchant_count_meet_scale_floor() {
    let loaded = bundled();
    let stats = corpus_stats(&loaded.examples);
    assert!(
        stats.total_examples >= 2000,
        "expected at least 2000 labeled examples, got {}",
        stats.total_examples
    );
    assert!(
        stats.unique_merchants >= 300,
        "expected at least 300 unique merchants, got {}",
        stats.unique_merchants
    );
}

#[test]
fn all_starter_categories_present_with_at_least_some_rows() {
    let loaded = bundled();
    let stats = corpus_stats(&loaded.examples);
    for cat in STARTER_CATEGORIES {
        let n = stats.category_distribution.get(*cat).copied().unwrap_or(0);
        assert!(n > 0, "starter category {cat:?} has zero rows in the multi-archetype corpus");
    }
}

#[test]
fn every_row_category_is_a_valid_starter_category() {
    let loaded = bundled();
    let valid: BTreeSet<&str> = STARTER_CATEGORIES.iter().copied().collect();
    for ex in &loaded.examples {
        assert!(
            valid.contains(ex.category.as_str()),
            "row {:?} has category {:?}, not one of the 10 starter categories",
            ex.id,
            ex.category
        );
    }
}

/// Per the task's explicit design requirement: dining and groceries are
/// naturally denser than travel and gifts in a realistic ledger, and this
/// corpus should NOT be artificially balanced to hide that.
#[test]
fn dining_and_groceries_have_more_rows_than_travel_and_gifts() {
    let loaded = bundled();
    let stats = corpus_stats(&loaded.examples);
    let dining = stats.category_distribution.get("dining").copied().unwrap_or(0);
    let groceries = stats.category_distribution.get("groceries").copied().unwrap_or(0);
    let travel = stats.category_distribution.get("travel").copied().unwrap_or(0);
    let gifts = stats.category_distribution.get("gifts").copied().unwrap_or(0);
    assert!(dining > travel, "dining ({dining}) should outnumber travel ({travel})");
    assert!(dining > gifts, "dining ({dining}) should outnumber gifts ({gifts})");
    assert!(groceries > travel, "groceries ({groceries}) should outnumber travel ({travel})");
    assert!(groceries > gifts, "groceries ({groceries}) should outnumber gifts ({gifts})");
}

/// Explicit exclusion #1 (transfers) and #2 (investment/brokerage), checked
/// directly against the committed file: zero rows may contain any of this
/// vocabulary. A grep-style substring scan, deliberately over-inclusive
/// (the full `FORBIDDEN_VOCAB` superset, not just the exact `is_transfer`
/// boolean formula) so this stays robust without re-deriving FinSight's
/// pairing/own-account-marker logic here.
#[test]
fn zero_transfer_or_investment_vocabulary_present() {
    let loaded = bundled();
    let mut violations: Vec<(String, String)> = Vec::new();
    for ex in &loaded.examples {
        let lower = ex.merchant_text.to_lowercase();
        for needle in FORBIDDEN_VOCAB {
            if lower.contains(needle) {
                violations.push((ex.id.clone(), (*needle).to_string()));
            }
        }
        // Also check against the REAL production predicate directly, not
        // just the mirrored keyword list above.
        assert!(
            !finsight_core::categorize::is_transfer(&ex.merchant_text),
            "row {:?} ({:?}) is transfer-shaped per the REAL is_transfer() predicate",
            ex.id,
            ex.merchant_text
        );
    }
    assert!(
        violations.is_empty(),
        "found {} forbidden-vocabulary violations, e.g. {:?}",
        violations.len(),
        &violations[..violations.len().min(10)]
    );
}

/// The declared keyword-hit ratio (see `eval/generate_synthetic_corpus.py`
/// module docstring: "Keyword-hit ratio: declared per category, not
/// organic") is a CHECKED claim, not a hope. This groups rows by
/// (category, merchant_id) — the ratio is a property of the MERCHANT
/// catalog design, not of transaction-volume-weighted row counts — and
/// calls the REAL `finsight_core::categorize::builtin_category` (never the
/// Python generator's mirror) to determine hit/no-hit per merchant, then
/// compares the measured rate to what the generator declared it would be.
#[test]
fn keyword_hit_ratio_matches_declared_target_per_category() {
    let loaded = bundled();

    // merchant_id -> (category, sample merchant_text)
    let mut merchants: BTreeMap<String, (String, String)> = BTreeMap::new();
    for ex in &loaded.examples {
        merchants
            .entry(ex.merchant_id.clone())
            .or_insert_with(|| (ex.category.clone(), ex.merchant_text.clone()));
    }

    // Consistency guard: every row of the SAME merchant should agree on
    // hit/no-hit (the generator is designed to guarantee this — see
    // `assert_variant_bucket_consistency` in generate_synthetic_corpus.py).
    // Verified here against the REAL builtin_category, independent of the
    // Python mirror.
    let mut by_merchant_texts: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for ex in &loaded.examples {
        by_merchant_texts.entry(ex.merchant_id.clone()).or_default().push(&ex.merchant_text);
    }
    for (mid, texts) in &by_merchant_texts {
        let first_hit = finsight_core::categorize::builtin_category(texts[0]).is_some();
        for t in texts {
            let this_hit = finsight_core::categorize::builtin_category(t).is_some();
            assert_eq!(
                this_hit, first_hit,
                "merchant {mid:?} has inconsistent builtin_category hit/no-hit across its own \
                 text variants (one variant lost or gained the keyword substring) — text {t:?}"
            );
        }
    }

    let mut hit_count: BTreeMap<&str, u32> = BTreeMap::new();
    let mut total_count: BTreeMap<&str, u32> = BTreeMap::new();
    for (_mid, (cat, sample_text)) in &merchants {
        let cat_key = STARTER_CATEGORIES
            .iter()
            .find(|c| *c == cat)
            .copied()
            .unwrap_or_else(|| panic!("unexpected category {cat:?}"));
        *total_count.entry(cat_key).or_insert(0) += 1;
        if finsight_core::categorize::builtin_category(sample_text).is_some() {
            *hit_count.entry(cat_key).or_insert(0) += 1;
        }
    }

    let mut failures = Vec::new();
    for (cat, declared) in DECLARED_HIT_RATIO {
        let total = *total_count.get(cat).unwrap_or(&0);
        assert!(total > 0, "category {cat:?} has no merchants at all in the corpus");
        let hit = *hit_count.get(cat).unwrap_or(&0);
        let measured = hit as f64 / total as f64;
        let diff = (measured - declared).abs();
        if diff > HIT_RATIO_TOLERANCE {
            failures.push(format!(
                "{cat}: declared {:.1}%, measured {:.1}% ({hit}/{total} merchants), diff {:.1}pp > tolerance {:.0}pp",
                declared * 100.0,
                measured * 100.0,
                diff * 100.0,
                HIT_RATIO_TOLERANCE * 100.0
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "keyword-hit ratio drifted outside declared tolerance:\n{}",
        failures.join("\n")
    );
}

/// The real stress test of the merchant-disjoint split at scale: ~330
/// distinct merchants (vs. the small seed's ~29), across categories with
/// very different merchant counts (housing: ~6, dining: ~55).
#[test]
fn merchant_disjoint_split_holds_at_scale() {
    let loaded = bundled();
    let (reference, holdout) = merchant_disjoint_split(&loaded.examples, 0.3, 42);
    assert!(!reference.is_empty());
    assert!(!holdout.is_empty());
    assert!(
        merchant_sets_disjoint(&reference, &holdout),
        "merchant-disjoint split leaked a merchant across halves at scale"
    );
    assert_eq!(reference.len() + holdout.len(), loaded.examples.len());
}

/// Runs the full harness end to end over the multi-archetype corpus.
/// Deliberately NOT pinned to an exact baseline (unlike the small seed's
/// `eval/categorization_baseline.synthetic.json`) — see this module's doc
/// comment for why a byte-exact pin on 2000+ rows would be counterproductive
/// here. Instead this asserts the report is internally consistent and
/// non-degenerate.
#[test]
fn harness_report_runs_and_produces_sane_numbers() {
    let loaded = bundled();
    let report = report::run(&loaded.examples, loaded.provenance, 0.3, 42);

    assert_eq!(report.corpus_provenance, CorpusProvenance::Synthetic);
    assert!(report.caveat.to_lowercase().contains("synthetic"));
    assert_eq!(report.sources.len(), 2, "expected builtin + rule sources");

    for source in &report.sources {
        assert!(source.full_corpus.n_predicted <= source.full_corpus.n_total);
        assert!(source.full_corpus.n_correct <= source.full_corpus.n_predicted);
        assert!(source.holdout_only.n_predicted <= source.holdout_only.n_total);
        assert!(source.holdout_only.n_correct <= source.holdout_only.n_predicted);
        if let Some(p) = source.full_corpus.precision {
            assert!((0.0..=1.0).contains(&p), "precision out of range: {p}");
        }
    }

    let builtin = report.sources.iter().find(|s| s.source == "builtin").unwrap();
    assert!(builtin.full_corpus.coverage > 0.0, "builtin must cover more than nothing");
    assert!(builtin.full_corpus.coverage < 1.0, "builtin must not cover everything");

    let dining = report.corpus_stats.category_distribution.get("dining").copied().unwrap_or(0);
    let groceries = report.corpus_stats.category_distribution.get("groceries").copied().unwrap_or(0);
    let travel = report.corpus_stats.category_distribution.get("travel").copied().unwrap_or(0);
    let gifts = report.corpus_stats.category_distribution.get("gifts").copied().unwrap_or(0);
    assert!(dining > travel && dining > gifts);
    assert!(groceries > travel && groceries > gifts);
}

/// Measures (and loosely gates) the runtime of the full load + validate +
/// merchant-disjoint-split + confusion-matrix + threshold-sweep pipeline
/// over the whole corpus — the task explicitly asks this be MEASURED, not
/// assumed fast. Run with `cargo test -p finsight-eval --release -- \
/// --nocapture full_pipeline_runtime_is_fast` to see the printed number;
/// the assertion itself uses a generous cap (dev machines vary) so it
/// fails only on an actual pathological regression, not routine variance.
#[test]
fn full_pipeline_runtime_is_fast() {
    let start = Instant::now();
    let loaded = bundled();
    let _report = report::run(&loaded.examples, loaded.provenance, 0.3, 42);
    let elapsed = start.elapsed();
    eprintln!(
        "full load+validate+split+confusion+threshold-sweep pipeline over {} rows / {} \
         merchants took {:?}",
        loaded.examples.len(),
        corpus_stats(&loaded.examples).unique_merchants,
        elapsed
    );
    assert!(
        elapsed.as_secs() < 10,
        "pipeline took {elapsed:?}, expected well under 10s for pure in-memory text processing \
         over a few thousand rows"
    );
}
