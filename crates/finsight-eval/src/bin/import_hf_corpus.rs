//! Converts the MIT-licensed HuggingFace dataset
//! `DoDataThings/us-bank-transaction-categories-v2` into this repo's labeled
//! corpus JSONL (issue #89's format).
//!
//! ```text
//! curl -sL https://huggingface.co/datasets/DoDataThings/us-bank-transaction-categories-v2/resolve/main/transactions-synthetic.csv -o /tmp/txns.csv
//! cargo run -p finsight-eval --bin import_hf_corpus -- /tmp/txns.csv eval/categorization_corpus.semi_synthetic.jsonl
//! ```
//!
//! # Why this dataset
//!
//! Every corpus in this repo until now was invented, which means it could not
//! test the one thing a semantic categorizer must actually do: read a REAL
//! merchant string. This one composes 500+ real merchant names (`PUBLIX`,
//! `WAL-MART #0006`, `PYPL*365 MARKET`, `REPUBLIC SERVICES`) into eight real US
//! bank statement formats, with store numbers, ACH `PPD ID:` traces, embedded
//! addresses and inconsistent casing. `CATEGORIZATION_CORPUS.md` named exactly
//! that formatting-sensitivity as the gap an invented corpus cannot cover.
//!
//! It is emitted as `provenance: semi-synthetic`, never `real` — see
//! [`CorpusProvenance::SemiSynthetic`]. No human labeled these and no ledger
//! produced them; only the merchant→category relationship is real.
//!
//! # The category mapping is lossy, and that is a finding
//!
//! The source has 17 categories; FinSight ships 10. Seven source categories are
//! DROPPED rather than mapped, and the two reasons are different:
//!
//! - `Transfer` and `Income` are dropped on principle. FinSight never
//!   categorizes transfers — that is a hard invariant from epic #74, enforced
//!   in every pass — and income is not spending. Mapping them anywhere would
//!   put rows in the corpus that production is required to refuse, so the
//!   harness would measure the categorizer failing at something it is correct
//!   to refuse.
//! - `Insurance`, `Education`, `Entertainment`, `Personal Care` and `Fees` are
//!   dropped because FinSight's starter set has no equivalent. Forcing them
//!   into the nearest neighbour would inject label noise and quietly measure
//!   the mapping rather than the categorizer.
//!
//! Dropping them makes the task EASIER (fewer confusable classes), so the
//! resulting precision is an optimistic bound relative to a ledger that
//! contains insurance and tuition. That the starter set cannot represent five
//! common spending categories is itself worth knowing — it is a product gap,
//! not a harness gap.

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

/// Source category → FinSight starter category id. Absent = deliberately
/// dropped; see the module doc for the two distinct reasons.
const CATEGORY_MAP: &[(&str, &str)] = &[
    ("Restaurants", "dining"),
    ("Groceries", "groceries"),
    ("Transportation", "transport"),
    ("Shopping", "shopping"),
    ("Travel", "travel"),
    ("Utilities", "utilities"),
    ("Subscription", "subscriptions"),
    ("Healthcare", "health"),
    // Both are shelter costs and FinSight has one category for them.
    ("Rent", "housing"),
    ("Mortgage", "housing"),
];

/// Merchant identity for SPLIT PURPOSES: the leading brand token.
///
/// # Why not `finsight_core::merchant::normalize_merchant`
///
/// That primitive is what production uses, and it is right for production's
/// job — grouping recurring charges, where keeping a location distinguishes two
/// real subscriptions. It is WRONG as a split key here, and measurably so:
/// against this dataset it yields **34 distinct ids for Publix**
/// (`publix philadelphia`, `publix columbus`, `publix market blvd`, …) because
/// the descriptors carry store numbers and addresses.
///
/// A merchant-disjoint split keyed on that is a fiction. The holdout would hold
/// `publix philadelphia` while the reference held `publix columbus`, the model
/// would only have to recognise the word PUBLIX, and precision would come out
/// inflated while looking perfectly rigorous. The whole point of the split is
/// to measure generalization to merchants never seen.
///
/// # Why the FIRST token, and what that costs
///
/// Taking one token guarantees one brand = one id, which is the property the
/// split needs. It over-collapses: `AMERICAN AIRLINES` and `AMERICAN EAGLE`
/// become one key. That is handled honestly downstream — any brand key mapping
/// to more than one category is DROPPED from the corpus entirely, rather than
/// contributing a coin-flip label. Losing those rows is a smaller sin than
/// silently training and scoring on contradictory ground truth.
fn brand_key(description: &str) -> String {
    let lower = description.to_ascii_lowercase();
    // Processor and channel prefixes carry no brand information, and leaving
    // them in would make every PayPal charge one giant "merchant".
    let stripped = lower
        .trim_start_matches("pos purchase")
        .trim_start_matches("preauthorized withdrawal to")
        .trim_start_matches("preauthorized withdrawal")
        .trim_start_matches("withdrawal from")
        .trim_start_matches("recurring payment to")
        .trim_start_matches("payment refund:")
        .trim_start_matches("payment to")
        .trim_start_matches("purchase authorized on")
        .trim_start_matches("mtg pmt")
        .trim();
    // `PYPL*365 MARKET`, `SQ *COFFEE`, `TST* DINER` — the brand is after the star.
    let stripped = stripped.rsplit('*').next().unwrap_or(stripped).trim();

    stripped
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .find(|tok| {
            // First token that actually looks like a name, not a store number
            // or an ACH artifact.
            !tok.is_empty() && tok.chars().any(|c| c.is_ascii_alphabetic()) && tok.len() > 1
        })
        .unwrap_or("")
        .to_string()
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args.next().context("usage: import_hf_corpus <input.csv> <output.jsonl>")?;
    let output = args.next().context("usage: import_hf_corpus <input.csv> <output.jsonl>")?;

    let map: BTreeMap<&str, &str> = CATEGORY_MAP.iter().copied().collect();
    let raw = std::fs::read_to_string(&input).with_context(|| format!("reading {input}"))?;

    let mut lines = raw.lines();
    let header = lines.next().context("empty csv")?;
    if !header.starts_with("description,category") {
        bail!("unexpected header {header:?}; expected `description,category`");
    }

    let mut kept = 0usize;
    let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_merchant: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = String::new();

    // First pass: which brand keys carry more than one category? `brand_key`
    // takes a single token, so `AMERICAN AIRLINES` (travel) and `AMERICAN
    // EAGLE` (shopping) collide. Those get dropped wholesale below rather than
    // contributing contradictory ground truth — a corpus that disagrees with
    // itself measures nothing, and does it invisibly.
    let mut brand_categories: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
    for line in raw.lines().skip(1) {
        let line = line.trim_end_matches('\r');
        let Some(split) = line.rfind(',') else { continue };
        let (description, category) = line.split_at(split);
        let category = category[1..].trim().trim_matches('"');
        let Some(mapped) = map.get(category) else { continue };
        let description = description
            .trim()
            .trim_matches('"')
            .trim_start_matches("[debit]")
            .trim_start_matches("[credit]")
            .trim();
        let key = brand_key(description);
        if key.is_empty() {
            continue;
        }
        brand_categories.entry(key).or_default().insert((*mapped).to_string());
    }
    let ambiguous: std::collections::BTreeSet<String> = brand_categories
        .iter()
        .filter(|(_, cats)| cats.len() > 1)
        .map(|(k, _)| k.clone())
        .collect();

    out.push_str("// provenance: semi-synthetic\n");
    out.push_str(
        "// Source: https://huggingface.co/datasets/DoDataThings/us-bank-transaction-categories-v2 (MIT)\n",
    );
    out.push_str("// Generated by `cargo run -p finsight-eval --bin import_hf_corpus`. Do not hand-edit.\n");

    for (i, line) in lines.enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        // The last comma separates description from category; descriptions can
        // contain commas (addresses do), categories cannot.
        let Some(split) = line.rfind(',') else { continue };
        let (description, category) = line.split_at(split);
        // Both fields arrive quoted (`"[debit] PUBLIX …","Groceries"`), so the
        // quotes have to come off BEFORE the category is looked up — leaving
        // them on silently drops every row as "unmapped", which is exactly what
        // happened the first time this ran.
        let category = category[1..].trim().trim_matches('"');

        let description = description.trim().trim_matches('"');
        // Strip the dataset's own `[debit] `/`[credit] ` sign prefix: FinSight
        // carries sign in `amount_cents`, never in `merchant_raw`, so leaving
        // it in would hand the encoder a signal production never sees.
        let description = description
            .trim_start_matches("[debit]")
            .trim_start_matches("[credit]")
            .trim();
        if description.is_empty() {
            continue;
        }

        let Some(mapped) = map.get(category) else {
            *dropped.entry(category.to_string()).or_insert(0) += 1;
            continue;
        };

        let merchant_id = brand_key(description);
        if merchant_id.trim().is_empty() {
            continue;
        }
        if ambiguous.contains(&merchant_id) {
            *dropped.entry(format!("<ambiguous brand: {merchant_id}>")).or_insert(0) += 1;
            continue;
        }
        *per_merchant.entry(merchant_id.clone()).or_insert(0) += 1;

        let example = finsight_eval::categorization::corpus::LabeledExample {
            id: format!("hf-{i}"),
            merchant_text: description.to_string(),
            merchant_id,
            category: (*mapped).to_string(),
            notes: None,
        };
        out.push_str(&serde_json::to_string(&example)?);
        out.push('\n');
        kept += 1;
    }

    std::fs::write(&output, out).with_context(|| format!("writing {output}"))?;

    eprintln!("kept {kept} rows across {} merchants -> {output}", per_merchant.len());
    eprintln!("dropped by category (no FinSight equivalent, or refused by invariant):");
    for (cat, n) in &dropped {
        eprintln!("  {cat:16} {n}");
    }
    let singletons = per_merchant.values().filter(|&&n| n == 1).count();
    eprintln!(
        "merchant sizes: {} singletons of {} merchants (singletons still split cleanly, they just \
         carry one row each)",
        singletons,
        per_merchant.len()
    );
    Ok(())
}
