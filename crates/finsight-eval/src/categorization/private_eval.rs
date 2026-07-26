//! Private, local-only precision-eval over a self-hosted instance's OWN real
//! `categorizations` corrections (`source = 'user'`) — the reframed
//! alternative to issue #89's literal ask (see the PR description for the
//! full reframe: a checked-in "real" labeled corpus cannot be honestly built
//! by an agent, and the repo owner's actual transactions must never become a
//! public artifact).
//!
//! Where [`super::corpus`] loads a checked-in JSONL file, this module reads
//! directly from the CALLING instance's own SQLCipher database: every row
//! here is that instance's own ground truth, produced when a human corrected
//! a transaction's category through the drawer, bulk edit, or the category
//! review queue. See the `source: "user".to_string()` write sites in
//! `crates/finsight-core/src/repos/transactions.rs`,
//! `crates/finsight-agent/src/categorizer.rs`, and
//! `crates/finsight-api/src/commands/transactions.rs`.
//!
//! This is real data (a human vouched for every label) without being a
//! privacy risk: nothing here talks to a network, writes a file the git repo
//! tracks, or is reachable from the Copilot/UI. It is meant to be called only
//! from a debug-only, admin-gated surface (see `finsight-server`'s
//! `/api/admin/private-category-eval` route) — never wired into
//! `bindings.ts` or any command the model can invoke.
//!
//! Reuses the SAME primitives the synthetic-corpus harness (issues #88/#89's
//! Slice 2/2b) already established, rather than inventing a second
//! merchant-disjoint-split or precision/coverage convention:
//! - [`super::split::merchant_disjoint_split`] for the merchant-disjoint
//!   holdout.
//! - [`super::confusion::ConfusionMatrix`] for precision/coverage.
//! - [`super::predictors::predict_builtin_for`] for the categorizer under
//!   measurement — the only real candidate today (see `super` module docs on
//!   why no `llm` baseline exists yet; a local sentence-encoder baseline is
//!   separate, out-of-scope work happening on another branch).
//! - `finsight_core::merchant::normalize_merchant` for merchant identity —
//!   the SAME grouping key categorization/recurring/insights already use
//!   throughout this codebase, not a bespoke normalization invented here.

use super::confusion::ConfusionMatrix;
use super::corpus::LabeledExample;
use super::predictors::predict_builtin_for;
use super::split::merchant_disjoint_split;
use rusqlite::Connection;
use std::collections::BTreeSet;
use std::fmt;

/// Matches the synthetic-corpus harness's own defaults
/// (`report.rs`'s `BASELINE_HOLDOUT_FRACTION`/`BASELINE_SEED`) so a private
/// real-data run and a synthetic run are at least methodologically
/// comparable.
pub const DEFAULT_HOLDOUT_FRACTION: f64 = 0.3;
pub const DEFAULT_SPLIT_SEED: u64 = 42;

/// Minimum number of DISTINCT held-out merchants before this tool reports a
/// precision number without a prominent "too small to trust" caveat.
///
/// **Rationale (a documented judgment call, not a proof).** The number this
/// tool reports is a proportion (correct / predicted) estimated over the
/// held-out merchants' transactions. A *merchant*, not a transaction, is the
/// unit that can leak information — a categorizer that nails one merchant's
/// three transactions has demonstrated one fact, not three — so the held-out
/// MERCHANT count is the real sample size, not the row count (this is the
/// same reasoning `split.rs` uses to split by merchant in the first place).
/// Below roughly 30 independent samples, a binomial proportion's
/// normal-approximation confidence interval is wide enough that a single
/// unlucky (or lucky) merchant swings the headline number by several points;
/// n≈30 is the textbook rule-of-thumb floor for treating a sampled
/// proportion as approximately normal (most treatments want np ≳ 10 and
/// n ≳ 30). That makes it a reasonable, defensible floor for "don't quote
/// this as a validated precision figure yet" — NOT a claim that n=30 buys a
/// narrow interval: at n=30 a proportion near 0.5 still carries a ~±18-point
/// 95% margin. The caveat text says exactly that ("too small for a
/// statistically meaningful claim"), and does not imply confidence once the
/// floor is cleared either. As real usage accumulates more corrections
/// across more distinct merchants, this number firms up on its own.
pub const MIN_HELDOUT_MERCHANTS_FOR_CONFIDENT_CLAIM: usize = 30;

/// Pull every real, currently-live user correction into the shared
/// [`LabeledExample`] shape the synthetic-corpus harness already uses, so the
/// exact same split/predict/score pipeline runs unmodified over real data.
///
/// - Only `source = 'user'` rows count as ground truth — `'rule'`/`'llm'`
///   rows are machine-generated categorizations, not corrections a human
///   vouched for.
/// - `category_id IS NOT NULL` — a NULL `category_id` means the user
///   CLEARED the category (see `V003__phase3_schema.sql`'s comment on the
///   column), not that they asserted a ground-truth label.
/// - Only the LATEST `source='user'` row per transaction — `categorizations`
///   is an append-only log, so a transaction corrected twice should
///   contribute its final verdict once, not both attempts as if they were
///   two independent data points.
/// - `merchant_id` is `finsight_core::merchant::normalize_merchant(merchant_raw)`
///   — the SAME grouping key categorization/recurring/insights already use
///   elsewhere in this codebase (`crates/finsight-core/src/merchant.rs`), not
///   a bespoke normalization invented for this tool.
pub fn fetch_user_corrections(conn: &Connection) -> rusqlite::Result<Vec<LabeledExample>> {
    let mut stmt = conn.prepare(
        "SELECT c.txn_id, c.category_id, t.merchant_raw \
         FROM categorizations c \
         JOIN transactions t ON t.id = c.txn_id \
         WHERE c.source = 'user' \
           AND c.category_id IS NOT NULL \
           AND c.at = ( \
             SELECT MAX(c2.at) FROM categorizations c2 \
             WHERE c2.txn_id = c.txn_id AND c2.source = 'user' \
           ) \
         ORDER BY c.txn_id",
    )?;
    let rows = stmt.query_map([], |r| {
        let txn_id: String = r.get(0)?;
        let category_id: String = r.get(1)?;
        let merchant_raw: String = r.get(2)?;
        Ok((txn_id, category_id, merchant_raw))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (txn_id, category_id, merchant_raw) = row?;
        let merchant_id = finsight_core::merchant::normalize_merchant(&merchant_raw);
        out.push(LabeledExample {
            id: txn_id,
            merchant_text: merchant_raw,
            merchant_id,
            category: category_id,
            notes: None,
        });
    }
    Ok(out)
}

fn distinct_merchants(examples: &[LabeledExample]) -> usize {
    examples.iter().map(|e| e.merchant_id.as_str()).collect::<BTreeSet<_>>().len()
}

/// The hard structural gate this tool relies on: a precision/coverage number
/// can only be constructed bundled with the N and merchant counts that
/// qualify it, and the ONLY way to read it back out is [`fmt::Display`],
/// which ALWAYS renders those counts — and the small-N caveat, when it
/// applies — alongside the percentage.
///
/// Fields are private, and this type deliberately does NOT derive
/// `Serialize`: a derive would reopen a JSON transport path where a caller
/// could pluck `precision` alone, exactly what this type exists to prevent.
/// A small set of N-only accessors are `pub` (reporting how much data went
/// in is never the sensitive part); precision/coverage have no public
/// accessor at all — `#[cfg(test)]`-only ones exist so unit tests can assert
/// the arithmetic without parsing the rendered string, but no production
/// code path can reach a bare percentage.
pub struct PrivateEvalResult {
    source: &'static str,
    precision: Option<f64>,
    coverage: f64,
    n_holdout_predicted: u64,
    n_holdout_correct: u64,
    n_total_corrections: usize,
    n_total_merchants: usize,
    n_holdout_corrections: usize,
    n_holdout_merchants: usize,
    holdout_fraction: f64,
    seed: u64,
}

impl PrivateEvalResult {
    /// True when the held-out merchant count falls below
    /// [`MIN_HELDOUT_MERCHANTS_FOR_CONFIDENT_CLAIM`] — the signal the
    /// `Display` impl uses to decide whether the prominent caveat renders.
    pub fn is_small_n(&self) -> bool {
        self.n_holdout_merchants < MIN_HELDOUT_MERCHANTS_FOR_CONFIDENT_CLAIM
    }

    /// Total `source='user'` corrections found (both halves of the split).
    pub fn n_total_corrections(&self) -> usize {
        self.n_total_corrections
    }

    /// Distinct merchants in the merchant-disjoint held-out slice — the
    /// number [`Self::is_small_n`] is gated on.
    pub fn n_holdout_merchants(&self) -> usize {
        self.n_holdout_merchants
    }

    #[cfg(test)]
    fn precision_for_test(&self) -> Option<f64> {
        self.precision
    }

    #[cfg(test)]
    fn coverage_for_test(&self) -> f64 {
        self.coverage
    }
}

impl fmt::Display for PrivateEvalResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Private local categorization precision-eval (source: {}) — computed from THIS \
             instance's own real corrections; never leaves this machine.",
            self.source
        )?;
        if self.n_total_corrections == 0 {
            return writeln!(
                f,
                "N=0 — this instance has no `source='user'` categorizations yet. Correct a few \
                 transaction categories (drawer, bulk edit, or the review queue) to start \
                 building a real precision signal."
            );
        }
        writeln!(
            f,
            "N = {} real user corrections across {} distinct merchants (this instance's whole \
             history); merchant-disjoint held-out eval slice = {} corrections across {} \
             distinct merchants (holdout_fraction={:.2}, split_seed={}).",
            self.n_total_corrections,
            self.n_total_merchants,
            self.n_holdout_corrections,
            self.n_holdout_merchants,
            self.holdout_fraction,
            self.seed,
        )?;
        if self.is_small_n() {
            writeln!(
                f,
                "CAVEAT: N={} held-out merchants is below this tool's {}-merchant floor for a \
                 statistically meaningful precision claim — a single merchant can swing the \
                 number below by several points. Treat the figure below as a rough signal only; \
                 it will firm up as you keep correcting categories and this instance's real \
                 corpus grows.",
                self.n_holdout_merchants, MIN_HELDOUT_MERCHANTS_FOR_CONFIDENT_CLAIM,
            )?;
        }
        match self.precision {
            Some(p) => write!(
                f,
                "precision = {:.1}% ({}/{} correct among predictions made), coverage = {:.1}% \
                 ({}/{} held-out transactions got a prediction at all)",
                p * 100.0,
                self.n_holdout_correct,
                self.n_holdout_predicted,
                self.coverage * 100.0,
                self.n_holdout_predicted,
                self.n_holdout_corrections,
            ),
            None => write!(
                f,
                "precision = undefined ({} made zero predictions on the held-out slice — 0.0% \
                 coverage)",
                self.source
            ),
        }
    }
}

/// Compute the private eval over an already-loaded set of real corrections.
/// Pure (no DB access), so it is directly unit-testable with hand-built
/// fixtures — [`run_private_eval`] is the thin DB-reading wrapper around
/// this.
pub fn evaluate_builtin_precision(
    examples: &[LabeledExample],
    holdout_fraction: f64,
    seed: u64,
) -> PrivateEvalResult {
    let n_total_corrections = examples.len();
    let n_total_merchants = distinct_merchants(examples);
    let (_reference, holdout) = merchant_disjoint_split(examples, holdout_fraction, seed);
    let n_holdout_corrections = holdout.len();
    let n_holdout_merchants = distinct_merchants(&holdout);
    let matrix = ConfusionMatrix::build("builtin", &holdout, predict_builtin_for);
    PrivateEvalResult {
        source: "builtin",
        precision: matrix.precision(),
        coverage: matrix.coverage(),
        n_holdout_predicted: matrix.n_predicted(),
        n_holdout_correct: matrix.n_correct(),
        n_total_corrections,
        n_total_merchants,
        n_holdout_corrections,
        n_holdout_merchants,
        holdout_fraction,
        seed,
    }
}

/// DB-reading entry point: pulls this instance's real corrections
/// ([`fetch_user_corrections`]) and runs [`evaluate_builtin_precision`] over
/// them with the module defaults. What an admin-gated surface calls.
pub fn run_private_eval(conn: &Connection) -> rusqlite::Result<PrivateEvalResult> {
    let examples = fetch_user_corrections(conn)?;
    Ok(evaluate_builtin_precision(&examples, DEFAULT_HOLDOUT_FRACTION, DEFAULT_SPLIT_SEED))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("private-eval.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    /// Seeds the fixed scaffolding every transaction needs (category group +
    /// one category per distinct category id used below, one account), then
    /// inserts a transaction + its `source='user'` categorization.
    fn seed_correction(
        conn: &Connection,
        txn_id: &str,
        merchant_raw: &str,
        category_id: &str,
        at: &str,
    ) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id,label,sort_order) VALUES('g','G',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO categories(id,group_id,label,color,sort_order) \
             VALUES(?1,'g',?1,'#f00',0)",
            [category_id],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('a1','Me','Bank','Checking','Ch','USD','#000','2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        // `OR IGNORE`: a test that corrects the SAME transaction twice (to
        // prove only-the-latest-wins) calls this helper twice with the same
        // `txn_id` — the transaction row itself must only be created once.
        conn.execute(
            "INSERT OR IGNORE INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES(?1,'a1','2024-01-01T00:00:00Z',-1000,?2,'cleared',0,'2024-01-01T00:00:00Z')",
            rusqlite::params![txn_id, merchant_raw],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
             VALUES(?1,?2,?3,'user',1.0,?4)",
            rusqlite::params![format!("cz-{txn_id}-{at}"), txn_id, category_id, at],
        )
        .unwrap();
    }

    // ── fetch_user_corrections: the DB-facing half ──────────────────────

    #[test]
    fn fetch_only_pulls_source_user_rows_with_a_real_category() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        seed_correction(&conn, "t-user", "Costco Wholesale", "groceries", "2024-01-01T00:00:00Z");
        // A machine-generated categorization on a different transaction —
        // must NOT be picked up as ground truth.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES('t-llm','a1','2024-01-01T00:00:00Z',-500,'Some Vendor','cleared',0,'2024-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
             VALUES('cz-llm','t-llm','groceries','llm',0.8,'2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let examples = fetch_user_corrections(&conn).unwrap();
        assert_eq!(examples.len(), 1, "only the source='user' row must be pulled");
        assert_eq!(examples[0].id, "t-user");
        assert_eq!(examples[0].category, "groceries");
    }

    #[test]
    fn fetch_excludes_cleared_categorizations_with_a_null_category_id() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('a1','Me','Bank','Checking','Ch','USD','#000','2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES('t-cleared','a1','2024-01-01T00:00:00Z',-500,'Some Vendor','cleared',0,'2024-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
             VALUES('cz-cleared','t-cleared',NULL,'user',1.0,'2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let examples = fetch_user_corrections(&conn).unwrap();
        assert!(
            examples.is_empty(),
            "a NULL category_id means the user CLEARED the category, not asserted a label"
        );
    }

    #[test]
    fn fetch_takes_only_the_latest_user_correction_per_transaction() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        seed_correction(&conn, "t1", "Costco Wholesale", "shopping", "2024-01-01T00:00:00Z");
        // The user changes their mind later — the LATER row is the real verdict.
        seed_correction(&conn, "t1", "Costco Wholesale", "groceries", "2024-06-01T00:00:00Z");

        let examples = fetch_user_corrections(&conn).unwrap();
        assert_eq!(examples.len(), 1, "one transaction must contribute exactly one example");
        assert_eq!(examples[0].category, "groceries", "the later correction is the final verdict");
    }

    #[test]
    fn fetch_derives_merchant_id_via_the_shared_normalize_merchant_primitive() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        seed_correction(
            &conn,
            "t1",
            "COSTCO WHOLESALE               VANCOUVER",
            "groceries",
            "2024-01-01T00:00:00Z",
        );
        let examples = fetch_user_corrections(&conn).unwrap();
        assert_eq!(
            examples[0].merchant_id,
            finsight_core::merchant::normalize_merchant("COSTCO WHOLESALE               VANCOUVER"),
            "merchant_id must come from the app's own shared normalization, not a bespoke one"
        );
    }

    // ── merchant-disjoint split, wired through the real DB path ─────────

    #[test]
    fn eval_over_real_corrections_stays_merchant_disjoint() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        // Several merchants, each with multiple transactions, so disjointness
        // actually exercises grouping (not just 1-row-per-merchant luck).
        let merchants = [
            ("Costco Wholesale", "groceries"),
            ("Best Buy", "shopping"),
            ("Riverbend Diner", "dining"),
            ("BrightGrid Hydro Payment", "utilities"),
            ("Oldtown Rentals Monthly Rent Payment", "housing"),
            ("Netflix.com", "subscriptions"),
        ];
        let mut n = 0;
        for (merchant, cat) in merchants {
            for row in 0..3 {
                n += 1;
                seed_correction(
                    &conn,
                    &format!("t-{n}"),
                    &format!("{merchant} #{row}"),
                    cat,
                    "2024-01-01T00:00:00Z",
                );
            }
        }

        let examples = fetch_user_corrections(&conn).unwrap();
        assert_eq!(examples.len(), 18);
        let (reference, holdout) = merchant_disjoint_split(&examples, 0.5, 7);
        assert!(!holdout.is_empty());
        assert!(!reference.is_empty());
        assert!(
            super::super::split::merchant_sets_disjoint(&reference, &holdout),
            "no merchant may appear on both sides of the split"
        );
        assert_eq!(reference.len() + holdout.len(), examples.len());
    }

    // ── precision arithmetic, pinned against a hand-computed fixture ────

    fn ex(id: &str, merchant_id: &str, merchant_text: &str, category: &str) -> LabeledExample {
        LabeledExample {
            id: id.into(),
            merchant_text: merchant_text.into(),
            merchant_id: merchant_id.into(),
            category: category.into(),
            notes: None,
        }
    }

    #[test]
    fn precision_and_coverage_are_arithmetically_correct_against_a_known_fixture() {
        // Four merchants, all forced into the holdout (holdout_fraction=1.0
        // sidesteps the hash-based selection so this test doesn't depend on
        // which merchants the split happens to pick):
        //   - Costco Wholesale: builtin predicts "groceries", label agrees   -> correct
        //   - Best Buy:         builtin predicts "shopping",  label agrees  -> correct
        //   - Oldtown Rentals…: builtin predicts "housing",   label is "dining" (WRONG on purpose)
        //   - Zzqxw Flibbert…:  no KEYWORD_MAP overlap -> abstains (does not count toward precision)
        let examples = vec![
            ex("1", "m-costco", "Costco Wholesale Uniq1", "groceries"),
            ex("2", "m-bestbuy", "Best Buy Uniq2", "shopping"),
            ex("3", "m-oldtown", "Oldtown Rentals Monthly Rent Payment Uniq3", "dining"),
            ex("4", "m-unknown", "Zzqxw Flibbertigibbet Emporium Uniq4", "dining"),
        ];
        // Preconditions: pin what the REAL builtin categorizer does on each
        // row, so the hand-computed expectation below is not a guess.
        assert_eq!(predict_builtin_for(&examples[0]).category.as_deref(), Some("groceries"));
        assert_eq!(predict_builtin_for(&examples[1]).category.as_deref(), Some("shopping"));
        assert_eq!(predict_builtin_for(&examples[2]).category.as_deref(), Some("housing"));
        assert_eq!(predict_builtin_for(&examples[3]).category, None, "must abstain: no keyword overlap");

        let result = evaluate_builtin_precision(&examples, 1.0, 42);

        // predicted = 3 (costco, bestbuy, oldtown), correct = 2 (costco, bestbuy).
        assert_eq!(
            result.precision_for_test(),
            Some(2.0 / 3.0),
            "precision must be exactly correct/predicted = 2/3"
        );
        // covered = 3 of 4 total held-out rows.
        assert!(
            (result.coverage_for_test() - 0.75).abs() < 1e-9,
            "coverage must be exactly predicted/total = 3/4, got {}",
            result.coverage_for_test()
        );
        assert_eq!(result.n_total_corrections(), 4);
    }

    #[test]
    fn zero_corrections_yields_undefined_precision_not_a_panic() {
        let result = evaluate_builtin_precision(&[], DEFAULT_HOLDOUT_FRACTION, DEFAULT_SPLIT_SEED);
        assert_eq!(result.precision_for_test(), None);
        assert_eq!(result.n_total_corrections(), 0);
        let rendered = result.to_string();
        assert!(rendered.contains("N=0"), "must say N=0 plainly, got: {rendered}");
    }

    // ── small-N caveat: fires below the floor, does not above it ────────

    #[test]
    fn small_n_caveat_fires_when_few_merchants_are_held_out() {
        // 6 distinct merchants; holdout_fraction=0.5 -> round(0.5*6)=3 held
        // out, well under the 30-merchant floor.
        let examples: Vec<_> = (0..6)
            .map(|i| ex(&format!("id{i}"), &format!("m-small-{i}"), &format!("Vendor{i}"), "dining"))
            .collect();
        let result = evaluate_builtin_precision(&examples, 0.5, 1);
        assert!(result.n_holdout_merchants() < MIN_HELDOUT_MERCHANTS_FOR_CONFIDENT_CLAIM);
        assert!(result.is_small_n());
        let rendered = result.to_string();
        assert!(
            rendered.contains("CAVEAT"),
            "a small held-out merchant count must render the prominent caveat, got: {rendered}"
        );
        assert!(
            rendered.contains(&result.n_holdout_merchants().to_string()),
            "the caveat must show the actual N, not just claim it's small, got: {rendered}"
        );
    }

    #[test]
    fn small_n_caveat_does_not_fire_at_or_above_the_merchant_floor() {
        // Exactly 30 distinct merchants, holdout_fraction=1.0 so all 30 land
        // in the held-out slice -> n_holdout_merchants == the floor exactly.
        let examples: Vec<_> = (0..MIN_HELDOUT_MERCHANTS_FOR_CONFIDENT_CLAIM)
            .map(|i| ex(&format!("id{i}"), &format!("m-big-{i}"), &format!("Vendor{i}"), "dining"))
            .collect();
        let result = evaluate_builtin_precision(&examples, 1.0, 1);
        assert_eq!(result.n_holdout_merchants(), MIN_HELDOUT_MERCHANTS_FOR_CONFIDENT_CLAIM);
        assert!(
            !result.is_small_n(),
            "a held-out merchant count at the floor must NOT be flagged as too small"
        );
        let rendered = result.to_string();
        assert!(
            !rendered.contains("CAVEAT"),
            "the prominent caveat must not render once the floor is cleared, got: {rendered}"
        );
    }

    /// A percentage never appears without its N somewhere on the same line
    /// or the line immediately around it — the closest a test can get to
    /// proving "no code path prints just the percentage alone" for the
    /// rendered text itself (the structural half of that guarantee is the
    /// private fields / no-`Serialize` design, enforced at compile time).
    #[test]
    fn rendered_report_always_carries_n_alongside_any_percentage() {
        let examples: Vec<_> = (0..6)
            .map(|i| ex(&format!("id{i}"), &format!("m-{i}"), "Costco Wholesale", "groceries"))
            .collect();
        let result = evaluate_builtin_precision(&examples, 0.5, 3);
        let rendered = result.to_string();
        if rendered.contains('%') {
            assert!(
                rendered.contains("N = ") || rendered.contains("N="),
                "a rendered report containing a percentage must also carry N, got: {rendered}"
            );
        }
    }
}
