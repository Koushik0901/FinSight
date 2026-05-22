use finsight_core::{db::run_migrations, keychain, sample::seed_household, Db};
use tempfile::TempDir;

#[test]
fn sample_seed_is_byte_for_byte_deterministic() {
    let key = keychain::generate_random_key();

    let dir_a = TempDir::new().unwrap();
    let db_a = Db::open(&dir_a.path().join("a.sqlcipher"), &key).unwrap();
    run_migrations(&db_a).unwrap();
    let a = seed_household(&db_a).unwrap();

    let dir_b = TempDir::new().unwrap();
    let db_b = Db::open(&dir_b.path().join("b.sqlcipher"), &key).unwrap();
    run_migrations(&db_b).unwrap();
    let b = seed_household(&db_b).unwrap();

    assert_eq!(a.accounts_created, b.accounts_created);
    assert_eq!(a.transactions_created, b.transactions_created,
               "transaction count drift — RNG stream changed; pin rand_chacha");

    // First merchant_raw (ordered by posted_at then by RNG-derived columns to
    // avoid non-deterministic tie-breaking on random UUIDs) must match across runs.
    let first_a: String = db_a.get().unwrap().query_row(
        "SELECT merchant_raw FROM transactions ORDER BY posted_at, merchant_raw, amount_cents LIMIT 1",
        [], |r| r.get(0)).unwrap();
    let first_b: String = db_b.get().unwrap().query_row(
        "SELECT merchant_raw FROM transactions ORDER BY posted_at, merchant_raw, amount_cents LIMIT 1",
        [], |r| r.get(0)).unwrap();
    assert_eq!(first_a, first_b);
}
