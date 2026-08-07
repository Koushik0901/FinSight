//! Merchant-disjoint split (issue #88's hard requirement, per the scoping
//! comment on epic #74): no merchant identity may appear in both halves of a
//! precision/coverage evaluation. Splitting by transaction instead of by
//! merchant would leak — a categorizer that has effectively memorized
//! "Brightloaf Grocers" from one transaction would trivially "generalize" to
//! another transaction from the exact same merchant, which proves nothing
//! about unseen merchants.

use super::corpus::LabeledExample;
use std::collections::{BTreeSet, HashSet};

/// FNV-1a, seeded. Chosen over pulling in `rand`/`rand_chacha` so this crate
/// stays dependency-light and CI-friendly (per CLAUDE.md's framing of
/// `finsight-eval` as a dev/CI tool) — this only needs a stable, deterministic
/// ranking of merchant ids, not cryptographic randomness.
fn stable_hash(s: &str, seed: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ seed;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Partition a labeled corpus into `(reference, holdout)` halves such that no
/// `merchant_id` appears in both — the merchant-disjoint split required by
/// issue #88. Deterministic for a given `seed` (same corpus + seed always
/// produces the same split, so a recorded baseline is reproducible).
///
/// `holdout_fraction` targets the fraction of **unique merchants** (not raw
/// transaction rows) placed in the holdout half, since disjointness is a
/// merchant-level property: a merchant with many transactions should not get
/// more "weight" toward crossing into holdout than a merchant with one.
///
/// Ranks merchants by `stable_hash(merchant_id, seed)` and takes the lowest
/// `round(holdout_fraction * n_merchants)` into holdout — deterministic and
/// proportion-correct even for small corpora, unlike a per-merchant coin-flip
/// which can land far from the target fraction when `n_merchants` is small.
pub fn merchant_disjoint_split(
    examples: &[LabeledExample],
    holdout_fraction: f64,
    seed: u64,
) -> (Vec<LabeledExample>, Vec<LabeledExample>) {
    assert!(
        (0.0..=1.0).contains(&holdout_fraction),
        "holdout_fraction must be in [0,1], got {holdout_fraction}"
    );

    let mut merchants: Vec<&str> = examples.iter().map(|e| e.merchant_id.as_str()).collect();
    merchants.sort_unstable();
    merchants.dedup();

    let mut ranked = merchants.clone();
    ranked.sort_by_key(|m| stable_hash(m, seed));

    let n_holdout = ((ranked.len() as f64) * holdout_fraction).round() as usize;
    let n_holdout = n_holdout.min(ranked.len());
    let holdout_merchants: HashSet<&str> = ranked.into_iter().take(n_holdout).collect();

    let mut reference = Vec::new();
    let mut holdout = Vec::new();
    for ex in examples {
        if holdout_merchants.contains(ex.merchant_id.as_str()) {
            holdout.push(ex.clone());
        } else {
            reference.push(ex.clone());
        }
    }
    (reference, holdout)
}

/// True iff no merchant id appears in both sets — the hard invariant the
/// split must uphold. Exposed standalone (not just inlined in
/// `merchant_disjoint_split`) so callers — and tests — can check ANY two
/// candidate partitions, including hand-constructed ones used to prove the
/// checker actually catches a violation (see the tests below).
pub fn merchant_sets_disjoint(a: &[LabeledExample], b: &[LabeledExample]) -> bool {
    let a_ids: BTreeSet<&str> = a.iter().map(|e| e.merchant_id.as_str()).collect();
    b.iter().all(|e| !a_ids.contains(e.merchant_id.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(id: &str, merchant_id: &str, category: &str) -> LabeledExample {
        LabeledExample {
            id: id.into(),
            merchant_text: format!("{merchant_id} purchase"),
            merchant_id: merchant_id.into(),
            category: category.into(),
            notes: None,
        }
    }

    /// The headline requirement from #88: "a test asserts zero merchant
    /// overlap between split halves." Uses a corpus where merchants have
    /// MULTIPLE transactions each (3 apiece), so this actually exercises
    /// grouping — a split that worked by accident on 1-transaction-per-
    /// merchant data would not prove the primitive groups correctly.
    #[test]
    fn split_is_merchant_disjoint_on_synthetic_corpus() {
        let mut examples = Vec::new();
        for i in 0..10 {
            let mid = format!("merchant-{i}");
            for j in 0..3 {
                examples.push(ex(&format!("{i}-{j}"), &mid, "groceries"));
            }
        }
        let (reference, holdout) = merchant_disjoint_split(&examples, 0.3, 42);

        assert!(
            !holdout.is_empty(),
            "expected a non-empty holdout at a 30% target"
        );
        assert!(!reference.is_empty(), "expected a non-empty reference half");
        assert!(
            merchant_sets_disjoint(&reference, &holdout),
            "reference and holdout must share zero merchants"
        );
        // Every example lands in exactly one half — none dropped, none duplicated.
        assert_eq!(reference.len() + holdout.len(), examples.len());
    }

    /// Verification step from the task: prove the disjointness checker would
    /// actually FAIL a split that leaks a merchant, not just vacuously pass
    /// because a real leak never occurs in these tests. Hand-constructs two
    /// sets that deliberately share `"shared-merchant"`.
    #[test]
    fn disjointness_checker_catches_a_deliberately_leaked_merchant() {
        let a = vec![
            ex("a1", "shared-merchant", "dining"),
            ex("a2", "only-a", "dining"),
        ];
        let b = vec![
            ex("b1", "shared-merchant", "dining"),
            ex("b2", "only-b", "dining"),
        ];
        assert!(
            !merchant_sets_disjoint(&a, &b),
            "checker must flag the merchant shared between a and b"
        );

        // Negative control: sets with genuinely no overlap must pass, so the
        // check above is proof the checker discriminates rather than always
        // returning false.
        let c = vec![ex("c1", "only-c", "dining")];
        assert!(
            merchant_sets_disjoint(&[a[1].clone()], &c),
            "'only-a' vs 'only-c' share no merchant and must pass"
        );
    }

    #[test]
    fn split_is_deterministic_for_a_fixed_seed() {
        let examples: Vec<_> = (0..20)
            .map(|i| ex(&format!("id{i}"), &format!("m{i}"), "dining"))
            .collect();
        let (r1, h1) = merchant_disjoint_split(&examples, 0.25, 7);
        let (r2, h2) = merchant_disjoint_split(&examples, 0.25, 7);
        let ids = |v: &[LabeledExample]| v.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            ids(&r1),
            ids(&r2),
            "reference half must be identical across runs"
        );
        assert_eq!(
            ids(&h1),
            ids(&h2),
            "holdout half must be identical across runs"
        );
    }

    #[test]
    fn different_seeds_can_produce_different_splits() {
        let examples: Vec<_> = (0..20)
            .map(|i| ex(&format!("id{i}"), &format!("m{i}"), "dining"))
            .collect();
        let (_, h_a) = merchant_disjoint_split(&examples, 0.3, 1);
        let (_, h_b) = merchant_disjoint_split(&examples, 0.3, 2);
        let ids = |v: &[LabeledExample]| {
            v.iter()
                .map(|e| e.merchant_id.clone())
                .collect::<BTreeSet<_>>()
        };
        assert_ne!(
            ids(&h_a),
            ids(&h_b),
            "different seeds should (almost always) pick a different holdout set"
        );
    }

    #[test]
    fn holdout_fraction_targets_unique_merchants_not_raw_rows() {
        // One merchant with 9 transactions, nine merchants with 1 each: 10
        // merchants total. A 50% merchant-level holdout should land near 5
        // merchants, regardless of the lopsided per-merchant row counts.
        let mut examples = Vec::new();
        for j in 0..9 {
            examples.push(ex(&format!("heavy-{j}"), "m-heavy", "groceries"));
        }
        for i in 0..9 {
            examples.push(ex(
                &format!("light-{i}"),
                &format!("m-light-{i}"),
                "groceries",
            ));
        }
        let (reference, holdout) = merchant_disjoint_split(&examples, 0.5, 99);
        let merchant_ids = |v: &[LabeledExample]| {
            v.iter()
                .map(|e| e.merchant_id.clone())
                .collect::<BTreeSet<_>>()
                .len()
        };
        let total_merchants = 10;
        let holdout_merchants = merchant_ids(&holdout);
        let reference_merchants = merchant_ids(&reference);
        assert_eq!(holdout_merchants + reference_merchants, total_merchants);
        // Exactly round(0.5 * 10) = 5 merchants in holdout.
        assert_eq!(holdout_merchants, 5);
    }

    #[test]
    #[should_panic(expected = "holdout_fraction must be in [0,1]")]
    fn rejects_out_of_range_fraction() {
        let examples = vec![ex("1", "m", "dining")];
        merchant_disjoint_split(&examples, 1.5, 0);
    }
}
