//! Confusion matrix: predicted vs. ground-truth category, broken out by
//! source, with precision/coverage derived from it.

use super::corpus::LabeledExample;
use super::predictors::Prediction;
use serde::Serialize;
use std::collections::BTreeMap;

/// The bucket key used for "this source declined to predict" — kept as an
/// explicit row in the matrix (rather than dropping abstentions silently) so
/// coverage is computable from the matrix's own counts alone.
pub const ABSTAIN_LABEL: &str = "<abstain>";

/// Predicted-vs-actual counts for one categorization source over one set of
/// labeled examples.
#[derive(Debug, Clone, Serialize)]
pub struct ConfusionMatrix {
    pub source: String,
    /// `counts[predicted_category_or_"<abstain>"][actual_category] = n`
    pub counts: BTreeMap<String, BTreeMap<String, u64>>,
    pub total: u64,
}

impl ConfusionMatrix {
    /// Runs `predict` over every example and tallies predicted-vs-actual.
    pub fn build(
        source: &str,
        examples: &[LabeledExample],
        predict: impl Fn(&LabeledExample) -> Prediction,
    ) -> Self {
        let mut counts: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
        for ex in examples {
            let pred = predict(ex);
            let key = pred.category.unwrap_or_else(|| ABSTAIN_LABEL.to_string());
            *counts
                .entry(key)
                .or_default()
                .entry(ex.category.clone())
                .or_insert(0) += 1;
        }
        Self {
            source: source.to_string(),
            counts,
            total: examples.len() as u64,
        }
    }

    /// Predictions actually made (excludes abstentions).
    pub fn n_predicted(&self) -> u64 {
        self.counts
            .iter()
            .filter(|(k, _)| k.as_str() != ABSTAIN_LABEL)
            .map(|(_, actuals)| actuals.values().sum::<u64>())
            .sum()
    }

    /// Correct predictions: predicted category equals actual category,
    /// excluding abstentions.
    pub fn n_correct(&self) -> u64 {
        self.counts
            .iter()
            .filter(|(k, _)| k.as_str() != ABSTAIN_LABEL)
            .map(|(predicted, actuals)| actuals.get(predicted).copied().unwrap_or(0))
            .sum()
    }

    /// Precision among predictions actually made: `correct / predicted`.
    /// `None` (not `0.0`) when nothing was predicted — undefined, not zero,
    /// so a 0-coverage source can't misleadingly read as "0% precision."
    pub fn precision(&self) -> Option<f64> {
        let predicted = self.n_predicted();
        if predicted == 0 {
            None
        } else {
            Some(self.n_correct() as f64 / predicted as f64)
        }
    }

    /// Fraction of all examples this source made a (non-abstain) prediction
    /// for.
    pub fn coverage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.n_predicted() as f64 / self.total as f64
        }
    }

    /// Per-category `(correct, total_actual)` breakdown — how many examples
    /// truly belonging to each category were correctly predicted, useful for
    /// spotting a category this source systematically misses or confuses.
    pub fn per_category_recall(&self) -> BTreeMap<String, (u64, u64)> {
        let mut totals: BTreeMap<String, u64> = BTreeMap::new();
        let mut correct: BTreeMap<String, u64> = BTreeMap::new();
        for (predicted, actuals) in &self.counts {
            for (actual, n) in actuals {
                *totals.entry(actual.clone()).or_insert(0) += n;
                if predicted == actual {
                    *correct.entry(actual.clone()).or_insert(0) += n;
                }
            }
        }
        totals
            .into_iter()
            .map(|(cat, total)| {
                (
                    cat.clone(),
                    (correct.get(&cat).copied().unwrap_or(0), total),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categorization::predictors::Prediction;

    fn ex(id: &str, merchant_id: &str, category: &str) -> LabeledExample {
        LabeledExample {
            id: id.into(),
            merchant_text: format!("{merchant_id} text"),
            merchant_id: merchant_id.into(),
            category: category.into(),
            notes: None,
        }
    }

    #[test]
    fn perfect_predictor_has_precision_1_and_full_coverage() {
        let examples = vec![ex("1", "m1", "dining"), ex("2", "m2", "groceries")];
        let matrix = ConfusionMatrix::build("test", &examples, |e| {
            Prediction::of(e.category.clone(), 1.0)
        });
        assert_eq!(matrix.precision(), Some(1.0));
        assert_eq!(matrix.coverage(), 1.0);
        assert_eq!(matrix.n_correct(), 2);
        assert_eq!(matrix.n_predicted(), 2);
    }

    #[test]
    fn always_abstain_has_zero_coverage_and_undefined_precision() {
        let examples = vec![ex("1", "m1", "dining")];
        let matrix = ConfusionMatrix::build("test", &examples, |_| Prediction::abstain());
        assert_eq!(matrix.coverage(), 0.0);
        assert_eq!(
            matrix.precision(),
            None,
            "precision must be undefined, not 0.0, with zero coverage"
        );
    }

    #[test]
    fn mixed_correct_incorrect_and_abstain() {
        // m1: predicts correctly. m2: predicts wrong category. m3: abstains.
        let examples = vec![
            ex("1", "m1", "dining"),
            ex("2", "m2", "groceries"),
            ex("3", "m3", "transport"),
        ];
        let matrix = ConfusionMatrix::build("test", &examples, |e| match e.merchant_id.as_str() {
            "m1" => Prediction::of("dining", 1.0),
            "m2" => Prediction::of("shopping", 1.0), // wrong on purpose
            _ => Prediction::abstain(),
        });
        assert_eq!(matrix.n_predicted(), 2, "m1 + m2 predicted, m3 abstained");
        assert_eq!(matrix.n_correct(), 1, "only m1 was correct");
        assert_eq!(matrix.precision(), Some(0.5));
        assert!((matrix.coverage() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn per_category_recall_breaks_out_by_actual_category() {
        let examples = vec![
            ex("1", "m1", "dining"),
            ex("2", "m2", "dining"),
            ex("3", "m3", "groceries"),
        ];
        let matrix = ConfusionMatrix::build("test", &examples, |e| match e.merchant_id.as_str() {
            "m1" => Prediction::of("dining", 1.0),   // correct
            "m2" => Prediction::of("shopping", 1.0), // wrong
            _ => Prediction::abstain(),              // m3 abstains
        });
        let recall = matrix.per_category_recall();
        assert_eq!(recall.get("dining"), Some(&(1, 2)));
        assert_eq!(recall.get("groceries"), Some(&(0, 1)));
    }
}
