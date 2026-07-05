//! Phase-attributed benchmarks for CSV import, over the real amex sample
//! (`samples/amex-all-time-statement.csv`, ~1988 rows). Each bench_function
//! isolates one phase of the import pipeline so we can see where time goes:
//!
//!   1. read_decode      — file read + layered decode (BOM sniff/UTF-8/1252)
//!   2. parse_only       — parse every data row into a ParsedRow (pure, no I/O)
//!   3. prepare_amex     — read + decode + parse + reconcile via the public,
//!                         read-only `CsvProvider::prepare()`, NO writes, over
//!                         a freshly seeded (empty-ledger) account per
//!                         iteration. This is the work the anticipatory
//!                         pipeline moves OFF the Import click. The gap
//!                         between this and import_amex_full is insert/commit
//!                         cost.
//!   4. import_amex_full — end-to-end `CsvProvider::import` (read, decode,
//!                         parse, reconcile, insert, commit) against a fresh
//!                         seeded DB per iteration.
//!   5. Five post-commit cascade steps, each benched individually against a
//!      DB that has already imported the amex file once:
//!         - categorize_builtin
//!         - pair_transfers
//!         - recompute_anomalies
//!         - net_worth_backfill
//!         - net_worth_record_today
//!
//! All DB-touching benches use `iter_batched` with fresh per-iteration state
//! (`BatchSize::LargeInput`) because these operations mutate the DB — reusing
//! a DB across iterations would measure the all-duplicates / no-op path from
//! iteration 2 onward, not the real cost.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use finsight_core::models::{AccountType, NewAccount};
use finsight_core::repos::accounts;
use finsight_core::Db;
use finsight_providers::csv::encoding::decode_layered;
use finsight_providers::csv::parse::parse_row;
use finsight_providers::csv::CsvProvider;
use finsight_providers::{AmountConvention, ColumnRole, CsvImportMapping};
use std::path::PathBuf;

fn repo_sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name)
}

/// Fresh temp-dir-backed SQLCipher DB with migrations applied.
fn fresh_db() -> (Db, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.sqlcipher"), &"cd".repeat(32)).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();
    (db, dir)
}

/// Seeds a single Credit-type account (amex-like) into `db`, returns its id.
fn seed_amex_account(db: &Db) -> String {
    let mut conn = db.get().unwrap();
    let account = accounts::insert(
        &mut conn,
        NewAccount {
            owner: "joint".into(),
            bank: "Amex".into(),
            r#type: AccountType::Credit,
            name: "Amex Card".into(),
            last4: Some("1001".into()),
            currency: "USD".into(),
            color: "#000000".into(),
            opening_balance_cents: 0,
            source: "manual".into(),
            liquidity_type: "illiquid".into(),
            emergency_fund_eligible: false,
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
            account_group: "credit".into(),
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
        },
    )
    .unwrap();
    account.id
}

/// Mapping for `samples/amex-all-time-statement.csv`.
/// Header: `Date,Date Processed,Description,Amount`.
/// Charges are positive (outflow), payments/credits are negative, so this is
/// PositiveIsOutflow. Column 2 ("Date Processed") is not used — Skip.
fn amex_mapping() -> CsvImportMapping {
    CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![
            ColumnRole::Date,
            ColumnRole::Skip,
            ColumnRole::Merchant,
            ColumnRole::Amount,
        ],
        date_format: "%d %b %Y".to_string(),
        amount_convention: AmountConvention::PositiveIsOutflow,
        decimal_separator: '.',
        delimiter: None,
    }
}

/// Sets up a fresh DB, seeds one account, imports the amex file once. Used as
/// the shared setup for the post-commit-cascade benches, which measure a
/// single call of one cascade step against DB state that already reflects a
/// completed import.
fn db_with_amex_imported() -> (Db, tempfile::TempDir, String) {
    let (db, dir) = fresh_db();
    let account_id = seed_amex_account(&db);
    let path = repo_sample("amex-all-time-statement.csv");
    let mapping = amex_mapping();
    let import_id = uuid::Uuid::new_v4().to_string();
    let summary = CsvProvider::import(&path, &account_id, &import_id, &mapping, &db, |_| {}).unwrap();
    assert!(
        summary.rows_imported > 1000,
        "sanity: expected a healthy majority of ~1988 amex rows to import, got {}",
        summary.rows_imported
    );
    (db, dir, account_id)
}

fn bench_read_decode(c: &mut Criterion) {
    let path = repo_sample("amex-all-time-statement.csv");
    c.bench_function("read_decode", |b| {
        b.iter(|| {
            let bytes = std::fs::read(&path).unwrap();
            let (text, _encoding) = decode_layered(&bytes).unwrap();
            criterion::black_box(text);
        });
    });
}

fn bench_parse_only(c: &mut Criterion) {
    let path = repo_sample("amex-all-time-statement.csv");
    let bytes = std::fs::read(&path).unwrap();
    let (text, _) = decode_layered(&bytes).unwrap();
    let mapping = amex_mapping();

    c.bench_function("parse_only", |b| {
        b.iter(|| {
            let mut reader = csv::ReaderBuilder::new()
                .has_headers(false)
                .delimiter(b',')
                .flexible(true)
                .from_reader(text.as_bytes());
            let mut parsed_count = 0u32;
            for (idx, rec) in reader.records().enumerate() {
                let rec = rec.unwrap();
                if idx < mapping.skip_header_rows as usize {
                    continue;
                }
                let fields: Vec<&str> = rec.iter().collect();
                if parse_row(&fields, &mapping).is_ok() {
                    parsed_count += 1;
                }
            }
            criterion::black_box(parsed_count);
        });
    });
}

/// prepare = read + decode + parse + reconcile, NO writes. Against an empty
/// ledger (fresh account). The gap between this and import_amex_full is the
/// insert/commit cost; this is the work the anticipatory pipeline moves OFF
/// the Import click.
fn bench_prepare_amex(c: &mut Criterion) {
    c.bench_function("prepare_amex", |b| {
        b.iter_batched(
            || {
                let (db, dir) = fresh_db();
                let id = seed_amex_account(&db);
                (db, dir, id)
            },
            |(db, _dir, account_id)| {
                let conn = db.get().unwrap();
                let path = repo_sample("amex-all-time-statement.csv");
                let mapping = amex_mapping();
                let p = CsvProvider::prepare(&path, &account_id, &mapping, &conn).unwrap();
                criterion::black_box(p);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_import_amex_full(c: &mut Criterion) {
    c.bench_function("import_amex_full", |b| {
        b.iter_batched(
            || {
                let (db, dir) = fresh_db();
                let account_id = seed_amex_account(&db);
                (db, dir, account_id)
            },
            |(db, _dir, account_id)| {
                let path = repo_sample("amex-all-time-statement.csv");
                let mapping = amex_mapping();
                let import_id = uuid::Uuid::new_v4().to_string();
                let summary =
                    CsvProvider::import(&path, &account_id, &import_id, &mapping, &db, |_| {})
                        .unwrap();
                criterion::black_box(summary);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_categorize_builtin(c: &mut Criterion) {
    c.bench_function("categorize_builtin", |b| {
        b.iter_batched(
            db_with_amex_imported,
            |(db, _dir, _account_id)| {
                let mut conn = db.get().unwrap();
                let n = finsight_core::categorize::apply_builtin_categorization(&mut conn).unwrap();
                criterion::black_box(n);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_pair_transfers(c: &mut Criterion) {
    c.bench_function("pair_transfers", |b| {
        b.iter_batched(
            db_with_amex_imported,
            |(db, _dir, _account_id)| {
                let mut conn = db.get().unwrap();
                let n = finsight_core::categorize::pair_transfers(&mut conn).unwrap();
                criterion::black_box(n);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_recompute_anomalies(c: &mut Criterion) {
    c.bench_function("recompute_anomalies", |b| {
        b.iter_batched(
            db_with_amex_imported,
            |(db, _dir, _account_id)| {
                let mut conn = db.get().unwrap();
                let n = finsight_core::anomaly::recompute_anomalies(&mut conn).unwrap();
                criterion::black_box(n);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_recompute_anomalies_scoped(c: &mut Criterion) {
    // Worst case for scoping: a single-account ledger, so the scoped pass
    // touches every merchant anyway. This must not regress vs. the full pass;
    // the real win is on multi-account ledgers (not modelled by the amex sample).
    c.bench_function("recompute_anomalies_scoped", |b| {
        b.iter_batched(
            db_with_amex_imported,
            |(db, _dir, account_id)| {
                let mut conn = db.get().unwrap();
                let n = finsight_core::anomaly::recompute_anomalies_for_account(
                    &mut conn,
                    &account_id,
                )
                .unwrap();
                criterion::black_box(n);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_net_worth_backfill(c: &mut Criterion) {
    c.bench_function("net_worth_backfill", |b| {
        b.iter_batched(
            db_with_amex_imported,
            |(db, _dir, _account_id)| {
                let mut conn = db.get().unwrap();
                finsight_core::repos::net_worth::backfill_history_from_transactions(&mut conn)
                    .unwrap();
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_net_worth_record_today(c: &mut Criterion) {
    c.bench_function("net_worth_record_today", |b| {
        b.iter_batched(
            db_with_amex_imported,
            |(db, _dir, _account_id)| {
                let mut conn = db.get().unwrap();
                finsight_core::repos::net_worth::record_today(&mut conn).unwrap();
            },
            BatchSize::LargeInput,
        );
    });
}

fn config() -> Criterion {
    // ~2k-row end-to-end import + 5 DB-setup-heavy cascade benches would be
    // very slow at the default 100 samples. 20 samples is enough for a
    // stable median at this row count.
    Criterion::default().sample_size(20)
}

criterion_group!(
    name = benches;
    config = config();
    targets =
        bench_read_decode,
        bench_parse_only,
        bench_prepare_amex,
        bench_import_amex_full,
        bench_categorize_builtin,
        bench_pair_transfers,
        bench_recompute_anomalies,
        bench_recompute_anomalies_scoped,
        bench_net_worth_backfill,
        bench_net_worth_record_today,
);
criterion_main!(benches);
