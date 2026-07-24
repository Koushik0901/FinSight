//! Per-source prediction functions. Each maps a merchant description string
//! to a [`Prediction`] — the common shape [`confusion`](super::confusion) and
//! [`threshold`](super::threshold) consume regardless of source.

use super::corpus::LabeledExample;

/// A predicted category (or an abstention) with a confidence score. `None`
/// category means "no confident match" — this source declined to predict,
/// which is different from predicting wrong and must stay distinguishable so
/// coverage is computable.
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction {
    pub category: Option<String>,
    pub confidence: f64,
}

impl Prediction {
    pub fn abstain() -> Self {
        Self { category: None, confidence: 0.0 }
    }

    pub fn of(category: impl Into<String>, confidence: f64) -> Self {
        Self { category: Some(category.into()), confidence }
    }
}

/// The `builtin` source: wraps the REAL production keyword matcher
/// (`finsight_core::categorize::builtin_category`) so this harness measures
/// the actual shipped lookup table, not a reimplementation. Confidence is
/// always `1.0` when it fires, matching production
/// (`finsight_core::categorize::apply_builtin_categorization` hardcodes
/// `confidence = 1.0` in every `categorizations` insert for `source =
/// 'builtin'`).
///
/// **Fidelity caveat — the remaining gaps all INFLATE, never deflate.** This
/// wraps `builtin_category`, not the full `apply_builtin_categorization` pass.
/// Every production gate this harness does not model is a gate that makes
/// production emit *nothing* where the harness emits a prediction. If that
/// prediction happens to match the row's label, the harness scores a point
/// production never earned — measured precision comes out **higher** than the
/// shipped pass would deliver. For a "≥98% precision" auto-apply gate,
/// inflation is the direction that matters.
///
/// Modeled here:
/// - **Transfer skip** (partially, see below): production does
///   `if treat_as_transfer { continue; }` — transfers are never categorized.
///   The guard below reproduces the `finsight_core::categorize::is_transfer`
///   half of that, which is the largest closable share.
///
/// Still unmodeled (each one inflating):
/// - `transfer_peer_id` pairing and `TransferContext::is_self_transfer`
///   (owner names / owned-bank aliases) — the other two inputs to production's
///   `treat_as_transfer`. Both need database state (a paired counter-leg, the
///   user's own identity), which a flat labeled corpus does not carry.
/// - The **category-existence gate**: production only assigns a category that
///   exists in the user's `categories` table. A user who deleted a starter
///   category gets nothing where this harness scores a hit.
/// - **Investment activity typing** (`activity_category`, which beats the
///   keyword map in production). Not modelable at all today: [`LabeledExample`]
///   has no `activity_type` field, so the corpus format would have to grow one
///   first.
///
/// See `eval/CATEGORIZATION_CORPUS.md` ("Fidelity to production") for the same
/// list in prose, which is what a contributor reads before this doc comment.
pub fn predict_builtin(merchant_text: &str) -> Prediction {
    // Abstain on transfer-shaped rows: production never categorizes a
    // transfer, so a category emitted here would be a prediction the shipped
    // pass does not make. Uses the SAME production predicate
    // (`finsight_core::categorize::is_transfer`) the real pass folds into
    // `treat_as_transfer`, so this tracks any future change to the keyword
    // lists automatically.
    if finsight_core::categorize::is_transfer(merchant_text) {
        return Prediction::abstain();
    }
    match finsight_core::categorize::builtin_category(merchant_text) {
        Some(cat) => Prediction::of(cat, 1.0),
        None => Prediction::abstain(),
    }
}

pub fn predict_builtin_for(example: &LabeledExample) -> Prediction {
    predict_builtin(&example.merchant_text)
}

/// One synthetic "user rule" for the `rule` source baseline. Real rules
/// (`rules` table) are per-user data with no generalizable content to seed
/// here — these exist purely so the harness can exercise the SAME matching
/// semantics a real rule pass uses, against this session's synthetic corpus.
/// See `eval/CATEGORIZATION_CORPUS.md`.
#[derive(Debug, Clone)]
pub struct SyntheticRule {
    pub pattern: String,
    pub category_id: &'static str,
}

/// A small, hand-picked rule set targeting merchants present in
/// `eval/categorization_corpus.synthetic.jsonl` (see that file's `notes`
/// fields for which rows each rule targets). NOT derived from any real user's
/// `rules` table.
pub fn synthetic_rules() -> Vec<SyntheticRule> {
    vec![
        SyntheticRule { pattern: "%brightloaf%".to_string(), category_id: "groceries" },
        SyntheticRule { pattern: "%cloudnote%".to_string(), category_id: "subscriptions" },
        SyntheticRule { pattern: "%craftbox%".to_string(), category_id: "shopping" },
        SyntheticRule { pattern: "%swiftcab%".to_string(), category_id: "transport" },
        SyntheticRule { pattern: "%riverbend diner%".to_string(), category_id: "dining" },
        SyntheticRule { pattern: "%thoughtful gifts%".to_string(), category_id: "gifts" },
    ]
}

/// Mirrors the LIKE-pattern matcher inlined in
/// `finsight_agent::categorizer::run_job`
/// (`crates/finsight-agent/src/categorizer.rs`, the `let matched =
/// active_rules.iter().find(...)` block): a wildcard-`%` prefix/suffix/contains
/// match, otherwise exact string equality, both case-insensitive.
///
/// Deliberately duplicated here rather than imported: `finsight-agent` pulls
/// in the full LLM provider stack (`CompletionProvider`, Ollama/OpenAI/
/// Anthropic HTTP clients), which this crate avoids so the harness stays
/// dependency-light and CI-friendly. If production's matcher semantics
/// change, update this copy too — flagged as a follow-up worth extracting to
/// a shared `finsight-core` function to remove the duplication risk.
fn rule_matches(pattern: &str, merchant_raw: &str) -> bool {
    let pat = pattern.to_lowercase();
    let merch = merchant_raw.to_lowercase();
    if pat.starts_with('%') && pat.ends_with('%') && pat.len() > 1 {
        merch.contains(&pat[1..pat.len() - 1])
    } else if let Some(stripped) = pat.strip_prefix('%') {
        merch.ends_with(stripped)
    } else if pat.ends_with('%') {
        merch.starts_with(&pat[..pat.len() - 1])
    } else {
        merch == pat
    }
}

/// The `rule` source: first matching rule wins (mirrors production, which
/// takes the first `active_rules` match). Confidence is always `1.0`,
/// matching production (`categorizer.rs` hardcodes `confidence: 1.0` for
/// `source: "rule"`).
pub fn predict_rule(merchant_text: &str, rules: &[SyntheticRule]) -> Prediction {
    match rules.iter().find(|r| rule_matches(&r.pattern, merchant_text)) {
        Some(r) => Prediction::of(r.category_id, 1.0),
        None => Prediction::abstain(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_fires_on_a_known_keyword() {
        let pred = predict_builtin("Costco Wholesale #331");
        assert_eq!(pred.category.as_deref(), Some("groceries"));
        assert_eq!(pred.confidence, 1.0);
    }

    #[test]
    fn builtin_abstains_on_no_overlap() {
        let pred = predict_builtin("Riverbend Diner");
        assert_eq!(pred, Prediction::abstain());
    }

    /// The inflation guard: a transfer-shaped descriptor that ALSO hits the
    /// keyword map must abstain, because production skips the row entirely
    /// (`treat_as_transfer` → `continue`) and never records a categorization.
    /// Without this, the harness would score such a row as a correct builtin
    /// prediction against any defensible label — measuring a precision the
    /// shipped pass does not deliver.
    #[test]
    fn builtin_abstains_on_transfer_shaped_rows_that_hit_the_keyword_map() {
        for descriptor in [
            // "autopay" is a UNILATERAL_TRANSFER_KEYWORD (credit-card autopay);
            // "netflix" is in KEYWORD_MAP → subscriptions.
            "AUTOPAY THANK YOU / NETFLIX.COM",
            // "internet withdrawal to" is a unilateral own-account marker;
            // "hydro" is in KEYWORD_MAP → utilities.
            "INTERNET WITHDRAWAL TO 00931 BC HYDRO",
            // pairing-eligible vocabulary + an explicit own-account marker,
            // which is the second arm of `is_transfer`; " rent " → housing.
            "INTERNET TRANSFER 106001023942 TO ACCOUNT 04930 RENT ",
        ] {
            // Precondition: the keyword map really does fire on this string,
            // so the abstention below is the guard doing work rather than a
            // vacuous "nothing matched anyway".
            assert!(
                finsight_core::categorize::builtin_category(descriptor).is_some(),
                "test descriptor {descriptor:?} must hit KEYWORD_MAP for this test to mean anything"
            );
            assert!(
                finsight_core::categorize::is_transfer(descriptor),
                "test descriptor {descriptor:?} must be transfer-shaped by production's own predicate"
            );
            assert_eq!(
                predict_builtin(descriptor),
                Prediction::abstain(),
                "transfer-shaped row {descriptor:?} must abstain — production never categorizes it"
            );
        }
    }

    /// Negative control: the guard must not swallow ordinary merchants that
    /// merely contain payment-ish words, or it would deflate coverage instead.
    #[test]
    fn builtin_still_fires_on_ordinary_merchants() {
        for (descriptor, expected) in [
            ("BrightGrid Hydro Payment", "utilities"),
            ("Oldtown Rentals Monthly Rent Payment", "housing"),
            ("Best Buy #2210", "shopping"),
        ] {
            assert_eq!(
                predict_builtin(descriptor).category.as_deref(),
                Some(expected),
                "{descriptor:?} is not a transfer and must still be categorized"
            );
        }
    }

    #[test]
    fn rule_contains_prefix_suffix_and_exact_semantics() {
        let rules = vec![
            SyntheticRule { pattern: "%brightloaf%".to_string(), category_id: "groceries" },
            SyntheticRule { pattern: "netflix.com".to_string(), category_id: "subscriptions" },
            SyntheticRule { pattern: "westgate%".to_string(), category_id: "health" },
            SyntheticRule { pattern: "%pharmacy".to_string(), category_id: "health" },
        ];
        assert_eq!(
            predict_rule("Brightloaf Grocers #12", &rules).category.as_deref(),
            Some("groceries"),
            "contains-wildcard should match"
        );
        assert_eq!(
            predict_rule("Netflix.com", &rules).category.as_deref(),
            Some("subscriptions"),
            "exact match (case-insensitive) should match"
        );
        assert_eq!(
            predict_rule("Westgate Pharmacy", &rules).category.as_deref(),
            Some("health"),
            "prefix wildcard should match"
        );
        assert_eq!(
            predict_rule("Downtown Pharmacy", &rules).category.as_deref(),
            Some("health"),
            "suffix wildcard should match"
        );
        assert_eq!(predict_rule("Unrelated Store", &rules), Prediction::abstain());
    }

    #[test]
    fn rule_first_match_wins() {
        let rules = vec![
            SyntheticRule { pattern: "%store%".to_string(), category_id: "shopping" },
            SyntheticRule { pattern: "%store%".to_string(), category_id: "groceries" },
        ];
        assert_eq!(predict_rule("Corner Store", &rules).category.as_deref(), Some("shopping"));
    }
}
