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
/// **Fidelity caveat:** this measures `builtin_category` in isolation, NOT
/// the full `apply_builtin_categorization` pass. Production additionally (a)
/// skips any row it treats as a transfer entirely (`if treat_as_transfer {
/// continue; }` — transfers are never categorized), (b) only assigns
/// categories that exist in the user's `categories` table, and (c) checks
/// `activity_category` (investment activity typing) before falling back to
/// the merchant keyword map. None of those gates are exercised by this
/// corpus (nothing here is transfer-shaped or an investment row), so the
/// numbers coincide today — but a real corpus containing e-transfer / card-
/// payment descriptors would need those gates modeled too, or "builtin
/// precision" from this harness would diverge from what the shipped pass
/// actually does.
pub fn predict_builtin(merchant_text: &str) -> Prediction {
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
