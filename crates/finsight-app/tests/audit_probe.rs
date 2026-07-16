//! AUDIT-ONLY probe (uncommitted, temporary). Drives the REAL import pipeline
//! against the user's actual bank CSVs in samples/, then dumps every derived
//! number (balances, cashflow, recurring, transfers, anomalies, net worth) so
//! the audit can diff them against independently computed ground truth.
//!
//! Run: cargo test -p finsight-app --release --test audit_probe -- --ignored --nocapture

use finsight_core::models::{AccountType, NewAccount};
use finsight_core::{db::run_migrations, keychain, metrics, Db};
use finsight_providers::csv::mapping::{AmountConvention, ColumnRole, CsvImportMapping};
use finsight_providers::CsvProvider;
use std::path::PathBuf;
use tempfile::TempDir;

fn bank_for(name: &str) -> &'static str {
    if name.starts_with("Amex") {
        "Amex"
    } else if name.starts_with("CIBC") {
        "CIBC"
    } else if name.starts_with("Wealthsimple") {
        "Wealthsimple"
    } else {
        "Tangerine"
    }
}

fn new_account(id_hint: &str, name: &str, ty: AccountType, ef: bool) -> NewAccount {
    let _ = id_hint;
    NewAccount {
        owner: "Koushik Sivarama Krishnan".into(),
        bank: bank_for(name).into(),
        r#type: ty,
        name: name.into(),
        last4: None,
        currency: "CAD".into(),
        color: "#888888".into(),
        opening_balance_cents: 0,
        source: "manual".into(),
        liquidity_type: "liquid".into(),
        emergency_fund_eligible: ef,
        goal_earmark: None,
        apy_pct: None,
        simplefin_account_id: None,
        nickname: None,
        connection_id: None,
        institution_id: None,
        external_account_id: None,
        official_name: None,
        mask: None,
        subtype: None,
        account_group: "cash".into(),
        available_balance_cents: None,
        balance_date: None,
        extra_json: None,
        raw_json: None,
        import_pending: false,
        apr_pct: None,
        min_payment_cents: None,
        payoff_date: None,
        limit_cents: None,
        original_balance_cents: None,
        started_at: None,
    }
}

struct Spec {
    file: &'static str,
    name: &'static str,
    ty: AccountType,
    ef: bool,
    mapping: CsvImportMapping,
}

fn specs() -> Vec<Spec> {
    use ColumnRole::*;
    vec![
        Spec {
            file: "amex-all-time-statement.csv",
            name: "Amex",
            ty: AccountType::Credit,
            ef: false,
            mapping: CsvImportMapping {
                skip_header_rows: 1,
                columns: vec![Date, Skip, Merchant, Amount],
                date_format: "%d %b %Y".into(),
                amount_convention: AmountConvention::PositiveIsOutflow,
                decimal_separator: '.',
                delimiter: None,
            },
        },
        Spec {
            file: "cibc-chequing-all-time-statement.csv",
            name: "CIBC Chequing",
            ty: AccountType::Checking,
            ef: false,
            mapping: CsvImportMapping {
                skip_header_rows: 0,
                columns: vec![Date, Merchant, Debit, Credit],
                date_format: "%Y-%m-%d".into(),
                amount_convention: AmountConvention::SplitDebitCredit,
                decimal_separator: '.',
                delimiter: None,
            },
        },
        Spec {
            file: "cibc-credit-card-all-time-statement.csv",
            name: "CIBC Credit",
            ty: AccountType::Credit,
            ef: false,
            mapping: CsvImportMapping {
                skip_header_rows: 0,
                columns: vec![Date, Merchant, Debit, Credit, Skip],
                date_format: "%Y-%m-%d".into(),
                amount_convention: AmountConvention::SplitDebitCredit,
                decimal_separator: '.',
                delimiter: None,
            },
        },
        Spec {
            file: "cibc-savings-all-time-statements.csv",
            name: "CIBC Savings",
            ty: AccountType::Savings,
            ef: true,
            mapping: CsvImportMapping {
                skip_header_rows: 0,
                columns: vec![Date, Merchant, Debit, Credit],
                date_format: "%Y-%m-%d".into(),
                amount_convention: AmountConvention::SplitDebitCredit,
                decimal_separator: '.',
                delimiter: None,
            },
        },
        Spec {
            file: "tangerine-chequing-all-time-statement.csv",
            name: "Tangerine Chequing",
            ty: AccountType::Checking,
            ef: false,
            mapping: CsvImportMapping {
                skip_header_rows: 1,
                columns: vec![Date, Skip, Merchant, Notes, Amount],
                date_format: "%m/%d/%Y".into(),
                amount_convention: AmountConvention::NegativeIsOutflow,
                decimal_separator: '.',
                delimiter: None,
            },
        },
        Spec {
            file: "tangerine-savings-all-time-statement.csv",
            name: "Tangerine Savings",
            ty: AccountType::Savings,
            ef: true,
            mapping: CsvImportMapping {
                skip_header_rows: 1,
                columns: vec![Date, Skip, Merchant, Notes, Amount],
                date_format: "%m/%d/%Y".into(),
                amount_convention: AmountConvention::NegativeIsOutflow,
                decimal_separator: '.',
                delimiter: None,
            },
        },
        // Brokerage activity statement (F1/F5): trades + contributions through
        // the same generic column mapping. Merchant = activity_sub_type
        // (EFT / E_TRFIN / BUY / SELL), Notes = security name, Amount =
        // net_cash_amount. The TFSA is invested, NOT emergency-fund-eligible.
        Spec {
            file: "wealthsimple-tfsa-all-time-statement.csv",
            name: "Wealthsimple TFSA",
            ty: AccountType::Investment,
            ef: false,
            mapping: CsvImportMapping {
                skip_header_rows: 1,
                columns: vec![
                    Date, Skip, Skip, Skip, Skip, Merchant, Skip, Skip, Notes, Skip, Skip, Skip,
                    Skip, Amount,
                ],
                date_format: "%Y-%m-%d".into(),
                amount_convention: AmountConvention::NegativeIsOutflow,
                decimal_separator: '.',
                delimiter: None,
            },
        },
    ]
}

#[test]
#[ignore]
fn audit_import_samples_and_dump_everything() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("audit.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    let samples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples");

    // 1. Create accounts + import each CSV through the REAL pipeline.
    let mut ids: Vec<(String, &'static str)> = Vec::new();
    for spec in specs() {
        let mut conn = db.get().unwrap();
        let acct =
            finsight_core::repos::accounts::insert(&mut conn, new_account("", spec.name, spec.ty, spec.ef))
                .unwrap();
        drop(conn);
        let import_id = uuid::Uuid::new_v4().to_string();
        let summary = CsvProvider::import(
            &samples.join(spec.file),
            &acct.id,
            &import_id,
            &spec.mapping,
            &db,
            |_| {},
        );
        match summary {
            Ok(s) => println!(
                "IMPORT {} -> imported={} skipped_dup={} queued={} errors={:?}",
                spec.name, s.rows_imported, s.rows_skipped_duplicates, s.rows_queued_for_review, s.errors
            ),
            Err(e) => println!("IMPORT {} -> ERROR {e}", spec.name),
        }
        ids.push((acct.id, spec.name));
    }

    // Operator identity (F0): a real user configures their own name, which
    // TransferContext loads so is_self_transfer recognizes their OWN e-transfers
    // ("INTERAC e-Transfer To/From: <me>"). We register only the operator, using
    // the first token of the account owner already present in the data — no
    // guessing at who else might be in the household. The household-member
    // detection itself (money to a partner reading as internal) is validated
    // GENERICALLY by the categorize unit test
    // `e_transfer_naming_a_household_member_is_a_self_transfer`, not by baking a
    // specific person into this real-data probe. Production stays generic — it
    // matches whatever names are configured, never a hard-coded one.
    {
        let conn = db.get().unwrap();
        let owner: String = conn
            .query_row("SELECT owner FROM accounts LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let first = owner.split_whitespace().next().unwrap_or(&owner);
        conn.execute(
            "INSERT INTO household_members(id,name,color,created_at) \
             VALUES('self', ?1, '#6366F1', datetime('now'))",
            [first],
        )
        .unwrap();
    }

    // 2. Post-import cascade exactly as the app runs it.
    {
        let mut conn = db.get().unwrap();
        let n = finsight_core::categorize::apply_builtin_categorization(&mut conn).unwrap();
        println!("CASCADE builtin_categorization changed={n:?}");
        let p = finsight_core::categorize::pair_transfers(&mut conn).unwrap();
        println!("CASCADE pair_transfers={p:?}");
        finsight_core::anomaly::recompute_anomalies(&mut conn).unwrap();
        for (id, _) in &ids {
            finsight_core::repos::accounts::recompute_balance_if_linked(&mut conn, id).unwrap();
        }
        finsight_core::repos::net_worth::record_today(&mut conn).unwrap();
        finsight_core::repos::net_worth::backfill_history_from_transactions(&mut conn).unwrap();
    }

    let mut conn = db.get().unwrap();

    // 3. Per-account facts.
    println!("\n== PER-ACCOUNT ==");
    for (id, name) in &ids {
        let (n, sum): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(amount_cents),0) FROM transactions WHERE account_id=?1",
                [id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let bal: Option<i64> = conn
            .query_row(
                "SELECT balance_cents FROM account_balances WHERE account_id=?1 ORDER BY as_of_date DESC LIMIT 1",
                [id.as_str()],
                |r| r.get(0),
            )
            .ok();
        let (min_d, max_d): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT MIN(substr(posted_at,1,10)), MAX(substr(posted_at,1,10)) FROM transactions WHERE account_id=?1",
                [id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        println!("{name}: txns={n} net_sum_cents={sum} latest_balance={bal:?} range={min_d:?}..{max_d:?}");
    }

    // 4. Global facts.
    println!("\n== GLOBAL ==");
    let (txn_total, transfers, anomalies, uncat): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM transactions),
                    (SELECT COUNT(*) FROM transactions WHERE is_transfer=1),
                    (SELECT COUNT(*) FROM transactions WHERE is_anomaly=1),
                    (SELECT COUNT(*) FROM transactions WHERE category_id IS NULL AND amount_cents<0 AND is_transfer=0)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    println!("txn_total={txn_total} transfers_flagged={transfers} anomalies={anomalies} uncategorized_expenses={uncat}");
    // Top uncategorized expense merchants (normalized) — evidence for whether the
    // remaining long tail is coverable by builtin keywords (P2-1).
    {
        use std::collections::HashMap;
        let mut counts: HashMap<String, (i64, i64)> = HashMap::new();
        let mut stmt = conn
            .prepare(
                "SELECT merchant_raw, amount_cents FROM transactions \
                 WHERE category_id IS NULL AND amount_cents < 0 AND is_transfer = 0",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap();
        for row in rows.flatten() {
            let key = finsight_core::merchant::canonical_merchant_key(&row.0);
            let e = counts.entry(key).or_insert((0, 0));
            e.0 += 1;
            e.1 += -row.1;
        }
        let mut v: Vec<_> = counts.into_iter().collect();
        v.sort_by_key(|(_, (_, spend))| -spend);
        println!("-- top uncategorized merchants (by spend) --");
        for (k, (n, spend)) in v.into_iter().take(25) {
            println!("  {spend:>10}c n={n:<4} {k}");
        }
    }
    let paired: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE transfer_peer_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(-1);
    println!("txns_with_transfer_peer={paired}");

    // TRANSFER-LEAK FORENSICS: inflow/outflow rows NOT flagged as transfers whose
    // merchant text screams "transfer". These leak straight into income/expense.
    println!("\n== UNFLAGGED TRANSFER-LIKE ROWS (leak into income/expense) ==");
    let mut stmt = conn
        .prepare(
            "SELECT merchant_raw, COUNT(*), SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END),
                    SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END)
             FROM transactions
             WHERE is_transfer=0 AND (
                   upper(merchant_raw) LIKE '%TRANSFER%'
                OR upper(merchant_raw) LIKE '%E-TRANSFER%'
                OR upper(merchant_raw) LIKE '%EFT%'
                OR upper(merchant_raw) LIKE '%PAYMENT RECEIVED%'
                OR upper(merchant_raw) LIKE '%AMERICAN EXPRESS%'
                OR upper(merchant_raw) LIKE '%FULFILL REQUEST%')
             GROUP BY 1 ORDER BY 3+4 DESC LIMIT 25",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
        })
        .unwrap();
    let mut leak_in = 0i64;
    let mut leak_out = 0i64;
    for row in rows.flatten() {
        println!("n={:<4} in={:>9} out={:>9}  {}", row.1, row.2, row.3, &row.0[..row.0.len().min(60)]);
        leak_in += row.2;
        leak_out += row.3;
    }
    println!("TOTAL unflagged transfer-like: inflow={leak_in} outflow={leak_out}");
    drop(stmt);

    // The user-facing "Possible transfers" review surface (F0 affordance): the
    // rows the Inbox/transactions filter would ask the user to rule on. The
    // whole leak above should be reachable through this surface (or already
    // categorized/decided) — an unreachable leak means the predicate rotted.
    let (review_n, review_in, review_out): (i64, i64, i64) = conn
        .query_row(
            &format!(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END),0),
                        COALESCE(SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END),0)
                 FROM transactions t WHERE {}",
                finsight_core::categorize::transfer_review_predicate("t")
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((-1, 0, 0));
    println!(
        "TRANSFER-REVIEW surface: n={review_n} inflow={review_in} outflow={review_out}"
    );
    assert!(
        review_n > 0,
        "samples/ contain undecided transfer-like rows; the review surface must list them"
    );
    {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT merchant_raw, COUNT(*), SUM(amount_cents)
                 FROM transactions t WHERE {}
                 GROUP BY 1 ORDER BY ABS(SUM(amount_cents)) DESC LIMIT 20",
                finsight_core::categorize::transfer_review_predicate("t")
            ))
            .unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
            })
            .unwrap();
        println!("-- review surface top merchants --");
        for row in rows.flatten() {
            println!("  n={:<4} sum={:>10}  {}", row.1, row.2, &row.0[..row.0.len().min(60)]);
        }
    }

    // Top raw inflows for May 2026 — what exactly is the app calling "income"?
    println!("\n== MAY 2026 'INCOME' ROWS (is_transfer=0, amount>0) top 15 ==");
    let mut stmt = conn
        .prepare(
            "SELECT substr(posted_at,1,10), merchant_raw, amount_cents FROM transactions
             WHERE is_transfer=0 AND amount_cents>0 AND posted_at>='2026-05-01' AND posted_at<'2026-06-01'
             ORDER BY amount_cents DESC LIMIT 15",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })
        .unwrap();
    for row in rows.flatten() {
        println!("{} {:>9}  {}", row.0, row.2, &row.1[..row.1.len().min(60)]);
    }
    drop(stmt);

    // Anomaly sample.
    println!("\n== ANOMALY SAMPLE (10) ==");
    let mut stmt = conn
        .prepare(
            "SELECT substr(posted_at,1,10), merchant_raw, amount_cents FROM transactions WHERE is_anomaly=1 ORDER BY ABS(amount_cents) DESC LIMIT 10",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })
        .unwrap();
    for row in rows.flatten() {
        println!("{} {:>9}  {}", row.0, row.2, &row.1[..row.1.len().min(60)]);
    }
    drop(stmt);

    // Category distribution after builtin pass.
    println!("\n== CATEGORY DISTRIBUTION (expenses, non-transfer) ==");
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(c.label,'(uncategorized)'), COUNT(*), SUM(-t.amount_cents)
             FROM transactions t LEFT JOIN categories c ON c.id=t.category_id
             WHERE t.amount_cents<0 AND t.is_transfer=0 GROUP BY 1 ORDER BY 3 DESC",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })
        .unwrap();
    for row in rows.flatten() {
        println!("{:<30} n={:<5} total_cents={}", row.0, row.1, row.2);
    }
    drop(stmt);

    // 5. Metrics layer.
    println!("\n== METRICS ==");
    let bb = metrics::balance_breakdown(&mut conn).unwrap();
    println!("balance_breakdown: {bb:?}");
    let ra = metrics::rolling_averages(&conn, 90).unwrap();
    println!("rolling_90: {ra:?}");
    let nw = finsight_core::repos::net_worth::breakdown(&mut conn).unwrap();
    println!("net_worth: {nw:?}");

    // F5: investment-account classification, proven on the real TFSA sample.
    {
        let tfsa_id = ids
            .iter()
            .find(|(_, n)| *n == "Wealthsimple TFSA")
            .map(|(id, _)| id.clone())
            .expect("TFSA spec imported");
        let (tfsa_n, tfsa_in, tfsa_out): (i64, i64, i64) = conn
            .query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END),0),
                        COALESCE(SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END),0)
                 FROM transactions WHERE account_id = ?1 AND is_transfer = 0",
                rusqlite::params![tfsa_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        println!("TFSA activity: n={tfsa_n} inflow={tfsa_in} outflow={tfsa_out}");
        assert!(tfsa_n > 0, "the brokerage CSV imports through the generic mapping");
        assert!(
            tfsa_in > 0 && tfsa_out > 0,
            "TFSA has real two-sided activity, so the exclusion below is exercised"
        );

        // Cashflow: brokerage activity is NOT income/spending. The metric must
        // equal the same sums restricted to non-investment accounts.
        let (m_inc, m_exp) = metrics::income_expense_since(&conn, "2000-01-01T00:00:00Z").unwrap();
        let (raw_inc, raw_exp): (i64, i64) = conn
            .query_row(
                "SELECT
                    COALESCE(SUM(CASE WHEN t.amount_cents>0 THEN t.amount_cents ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN t.amount_cents<0 THEN -t.amount_cents ELSE 0 END),0)
                 FROM transactions t JOIN accounts a ON a.id = t.account_id
                 WHERE t.is_transfer = 0 AND a.type != 'Investment'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(m_inc, raw_inc, "income metric excludes investment-account activity");
        assert_eq!(m_exp, raw_exp, "expense metric excludes investment-account activity");

        // Balances: the entered market value lands in `invested`, never in
        // liquid or the emergency fund, and net worth includes it.
        let before = metrics::balance_breakdown(&mut conn).unwrap();
        finsight_core::repos::accounts::set_current_balance(&mut conn, &tfsa_id, 1_234_500)
            .unwrap();
        let after = metrics::balance_breakdown(&mut conn).unwrap();
        println!("balance_breakdown after TFSA market value: {after:?}");
        assert_eq!(
            after.invested_cents,
            before.invested_cents + 1_234_500,
            "market value entered verbatim into the invested bucket"
        );
        assert_eq!(after.liquid_cents, before.liquid_cents, "TFSA is not liquid");
        assert_eq!(
            after.emergency_fund_cents, before.emergency_fund_cents,
            "TFSA is not emergency-fund-eligible"
        );
        assert_eq!(
            after.net_worth_cents,
            before.net_worth_cents + 1_234_500,
            "net worth includes the market value"
        );
    }

    // DECOMPOSITION (F0): what actually drives the rolling-90 expense? The top
    // expense rows in the window, so we can see whether the inflated number is
    // transfer leakage or REAL one-off spend BEFORE writing transfer-detection
    // code — otherwise we'd tune matching until the headline number flatters the
    // user, which is sample-fitting the user's own CSVs (the goal forbids it).
    println!("\n== TOP 25 EXPENSE ROWS in rolling-90 window (is_transfer=0, amount<0) ==");
    {
        let mut stmt = conn
            .prepare(
                "SELECT substr(t.posted_at,1,10), t.amount_cents, a.name, COALESCE(c.label,'(uncat)'), t.merchant_raw \
                 FROM transactions t JOIN accounts a ON a.id=t.account_id \
                 LEFT JOIN categories c ON c.id=t.category_id \
                 WHERE t.is_transfer=0 AND t.amount_cents<0 AND date(t.posted_at) >= date('now','-90 days') \
                 ORDER BY t.amount_cents ASC LIMIT 25",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .unwrap();
        let mut sum = 0i64;
        for row in rows {
            let (d, amt, acct, cat, m) = row.unwrap();
            sum += amt;
            println!(
                "{d} {amt:>9}c {:<16} {:<12} {}",
                acct.chars().take(16).collect::<String>(),
                cat.chars().take(12).collect::<String>(),
                m.chars().take(46).collect::<String>()
            );
        }
        println!("(top-25 expense subtotal = {sum}c)");
        let (counted, excluded): (i64, i64) = conn
            .query_row(
                "SELECT COALESCE(SUM(CASE WHEN is_transfer=0 AND amount_cents<0 THEN -amount_cents ELSE 0 END),0), \
                        COALESCE(SUM(CASE WHEN is_transfer=1 AND amount_cents<0 THEN -amount_cents ELSE 0 END),0) \
                 FROM transactions WHERE date(posted_at) >= date('now','-90 days')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        println!(
            "window expense: counted(is_transfer=0)={counted}c  already-excluded(is_transfer=1)={excluded}c"
        );
    }

    // Month cashflow for the 3 most recent full months present in data.
    println!("\n== MONTHLY CASHFLOW (metrics::cashflow_between) ==");
    for (start, end, label) in [
        ("2026-04-01", "2026-05-01", "2026-04"),
        ("2026-05-01", "2026-06-01", "2026-05"),
        ("2026-06-01", "2026-07-01", "2026-06"),
    ] {
        let cf = metrics::cashflow_between(&conn, start, end).unwrap();
        println!("{label}: {cf:?}");
    }

    // 6. Recurring detection.
    println!("\n== RECURRING (window 400d) ==");
    match finsight_core::recurring::detect_recurring(&conn, 400) {
        Ok(items) => {
            println!("count={}", items.len());
            for it in items {
                println!(
                    "kind={:<13?} conf={:.2} cadence={:<10} gap={:>5.1} last={:>9}c n={:<3} {}",
                    it.kind, it.confidence, it.cadence, it.avg_gap_days, it.last_amount_cents, it.occurrences, it.merchant_key
                );
            }
        }
        Err(e) => println!("ERROR {e}"),
    }

    // 6b. P0-2: assign owners to the REAL sample accounts and confirm per-member
    // flows reconcile to the household total on real data (Amex+CIBC → A,
    // Tangerine → B, CIBC Savings joint).
    println!("\n== PER-MEMBER RECONCILIATION (P0-2) ==");
    {
        use finsight_core::repos::household;
        let mut c = db.get().unwrap();
        let a = household::create_member(&mut c, "Person A", None).unwrap();
        let b = household::create_member(&mut c, "Person B", None).unwrap();
        for (id, name) in &ids {
            let owners: Vec<String> = if name.starts_with("Tangerine") {
                vec![b.id.clone()]
            } else if *name == "CIBC Savings" {
                vec![a.id.clone(), b.id.clone()] // joint
            } else {
                vec![a.id.clone()]
            };
            household::set_account_owners(&mut c, id, &owners).unwrap();
        }
        let start = "2000-01-01T00:00:00Z";
        let (h_inc, h_exp) = metrics::income_expense_since_for(&c, start, None).unwrap();
        let (a_inc, a_exp) = metrics::income_expense_since_for(&c, start, Some(a.id.as_str())).unwrap();
        let (b_inc, b_exp) = metrics::income_expense_since_for(&c, start, Some(b.id.as_str())).unwrap();
        println!("household inc={h_inc} exp={h_exp}");
        println!("A inc={a_inc} exp={a_exp} | B inc={b_inc} exp={b_exp}");
        println!(
            "A+B inc={} exp={} (expect ~household; 1 joint acct → ≤1c drift/aggregate)",
            a_inc + b_inc,
            a_exp + b_exp
        );
        let inc_drift = (a_inc + b_inc - h_inc).abs();
        let exp_drift = (a_exp + b_exp - h_exp).abs();
        assert!(inc_drift <= 2, "per-member income reconciles (drift={inc_drift})");
        assert!(exp_drift <= 2, "per-member expense reconciles (drift={exp_drift})");
        println!("reconciles: income drift={inc_drift}c, expense drift={exp_drift}c");
    }

    // 7. Net worth history sanity (backfill result).
    println!("\n== NET WORTH HISTORY (last 8 points) ==");
    let mut stmt = conn
        .prepare("SELECT date,total_cents FROM net_worth_snapshots ORDER BY date DESC LIMIT 8")
        .unwrap();
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .unwrap();
    for row in rows.flatten() {
        println!("{} {}", row.0, row.1);
    }
    drop(stmt);

    // 8. RE-IMPORT EVERY file (dedup audit): a clean re-import of an already
    // imported statement must insert nothing and queue nothing — every incoming
    // row is an exact twin, so bipartite set-matching pairs them 1:1. This is
    // read-only via `prepare` so it does not perturb the ledger between files.
    println!("\n== RE-IMPORT DEDUP CHECK (all samples) ==");
    drop(conn);
    for (spec, (acct_id, _name)) in specs().iter().zip(ids.iter()) {
        let conn2 = db.get().unwrap();
        let prepared =
            CsvProvider::prepare(&samples.join(spec.file), acct_id, &spec.mapping, &conn2).unwrap();
        println!(
            "re-prepare {} -> imported={} skipped={} queued={} (expect imported=0, queued=0)",
            spec.name,
            prepared.rows_imported,
            prepared.rows_skipped_duplicates,
            prepared.rows_queued_for_review
        );
        // Forensics: dump any rows that still queue (should be none).
        let mut shown = 0;
        for row in &prepared.rows {
            if let finsight_providers::csv::prepare::PreparedDecision::Review {
                candidate,
                matches,
                confidence,
                reason,
            } = &row.decision
            {
                if shown < 8 {
                    let top = matches.first();
                    println!(
                        "  QUEUED {} row#{} cand[{}c '{}' @{}] conf={} reason='{}' -> best[{}c '{}' score={}] n={}",
                        spec.name,
                        row.row_number,
                        candidate.amount_cents,
                        candidate.merchant_raw,
                        candidate.posted_at.date_naive(),
                        confidence,
                        reason,
                        top.map(|t| t.transaction.amount_cents).unwrap_or(0),
                        top.map(|t| t.transaction.merchant_raw.as_str()).unwrap_or(""),
                        top.map(|t| t.score).unwrap_or(0),
                        matches.len(),
                    );
                }
                shown += 1;
            }
        }
        assert_eq!(prepared.rows_imported, 0, "{}: re-import must insert nothing", spec.name);
        assert_eq!(prepared.rows_queued_for_review, 0, "{}: re-import must queue nothing", spec.name);
    }

    // 9. Reset (Delete All) then verify empty.
    println!("\n== RESET CHECK ==");
    let mut conn = db.get().unwrap();
    match finsight_core::repos::reset::delete_all_data(&mut conn) {
        Ok(_) => {
            let left: i64 = conn
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM transactions)+(SELECT COUNT(*) FROM accounts)+(SELECT COUNT(*) FROM net_worth_snapshots)",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            println!("after delete_all_data leftover_rows={left} (expect 0)");
        }
        Err(e) => println!("reset ERROR {e}"),
    }
}
