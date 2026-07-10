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

fn new_account(id_hint: &str, name: &str, ty: AccountType, ef: bool) -> NewAccount {
    let _ = id_hint;
    NewAccount {
        owner: "You".into(),
        bank: "Bank".into(),
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

    // 8. RE-IMPORT the first file (dedup audit): everything should skip.
    println!("\n== RE-IMPORT DEDUP CHECK (amex) ==");
    drop(conn);
    let spec0 = &specs()[0];
    let import_id = uuid::Uuid::new_v4().to_string();
    let s = CsvProvider::import(
        &samples.join(spec0.file),
        &ids[0].0,
        &import_id,
        &spec0.mapping,
        &db,
        |_| {},
    )
    .unwrap();
    println!(
        "re-import amex: imported={} skipped_dup={} queued={} (expect imported=0)",
        s.rows_imported, s.rows_skipped_duplicates, s.rows_queued_for_review
    );

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
