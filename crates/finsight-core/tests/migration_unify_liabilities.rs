//! Verifies V039 (unify `liabilities` into `accounts`) against a manually
//! rebuilt pre-V039 schema — full `run_migrations()` already includes V039,
//! so by the time it runs the `liabilities` table is gone; this replays the
//! exact historical DDL up through V038 to get a genuine "about to upgrade"
//! database, then re-executes V039's own SQL text against it.

use rusqlite::{params, Connection};

fn pre_v039_schema(conn: &Connection) {
    conn.execute_batch(
        "
        CREATE TABLE accounts (
          id           TEXT PRIMARY KEY,
          owner        TEXT NOT NULL,
          bank         TEXT NOT NULL,
          type         TEXT NOT NULL,
          name         TEXT NOT NULL,
          last4        TEXT,
          currency     TEXT NOT NULL DEFAULT 'USD',
          color        TEXT NOT NULL,
          archived_at  TEXT,
          created_at   TEXT NOT NULL,
          source TEXT NOT NULL DEFAULT 'manual',
          liquidity_type TEXT NOT NULL DEFAULT 'liquid',
          emergency_fund_eligible INTEGER NOT NULL DEFAULT 1,
          goal_earmark TEXT,
          apy_pct REAL,
          simplefin_account_id TEXT,
          last_synced_at TEXT,
          nickname TEXT,
          connection_id TEXT,
          institution_id TEXT,
          external_account_id TEXT,
          official_name TEXT,
          mask TEXT,
          subtype TEXT,
          account_group TEXT NOT NULL DEFAULT 'other',
          available_balance_cents INTEGER,
          balance_date TEXT,
          extra_json TEXT,
          raw_json TEXT,
          import_pending INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE account_balances (
          account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
          as_of_date TEXT NOT NULL,
          balance_cents INTEGER NOT NULL,
          available_balance_cents INTEGER,
          source TEXT NOT NULL DEFAULT 'manual',
          PRIMARY KEY (account_id, as_of_date, source)
        );

        CREATE TABLE liabilities (
          id             TEXT PRIMARY KEY,
          name           TEXT NOT NULL,
          liability_type TEXT NOT NULL,
          balance_cents  INTEGER NOT NULL DEFAULT 0,
          limit_cents    INTEGER,
          apr_pct        REAL,
          payoff_date    TEXT,
          currency       TEXT NOT NULL DEFAULT 'USD',
          created_at     TEXT NOT NULL,
          updated_at     TEXT NOT NULL,
          min_payment_cents INTEGER,
          original_balance_cents INTEGER,
          started_at TEXT
        );

        CREATE TABLE goals (
          id            TEXT PRIMARY KEY,
          name          TEXT NOT NULL,
          type          TEXT NOT NULL DEFAULT 'save-by-date',
          target_cents  INTEGER NOT NULL DEFAULT 0,
          current_cents INTEGER NOT NULL DEFAULT 0,
          monthly_cents INTEGER NOT NULL DEFAULT 0,
          target_date   TEXT,
          color         TEXT NOT NULL DEFAULT '#C9F950',
          notes         TEXT,
          sort_order    INTEGER NOT NULL DEFAULT 0,
          archived_at   TEXT,
          created_at    TEXT NOT NULL,
          liability_id TEXT REFERENCES liabilities(id) ON DELETE SET NULL,
          account_id  TEXT REFERENCES accounts(id)  ON DELETE SET NULL
        );
        ",
    )
    .unwrap();
}

fn read_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations")
        .join("V039__unify_liabilities_into_accounts.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn run_v039(conn: &Connection) {
    let sql = read_migration_sql();
    let stripped: String = sql
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for stmt in stripped.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        conn.execute(trimmed, []).unwrap();
    }
}

#[test]
fn folds_every_liability_type_into_the_matching_account_type_and_color() {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    pre_v039_schema(&conn);

    for (id, ltype) in [
        ("cc", "credit-card"),
        ("mort", "mortgage"),
        ("ln", "loan"),
        ("oth", "other"),
        ("weird", "not-a-real-type"),
    ] {
        conn.execute(
            "INSERT INTO liabilities(id, name, liability_type, balance_cents, currency, created_at, updated_at) \
             VALUES(?1, ?1, ?2, 100000, 'USD', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            params![id, ltype],
        )
        .unwrap();
    }

    run_v039(&conn);

    let read_type_color = |id: &str| -> (String, String) {
        conn.query_row(
            "SELECT type, color FROM accounts WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };
    assert_eq!(read_type_color("cc"), ("Credit".into(), "#F97316".into()));
    assert_eq!(read_type_color("mort"), ("Loan".into(), "#F87171".into()));
    assert_eq!(read_type_color("ln"), ("Loan".into(), "#F87171".into()));
    assert_eq!(read_type_color("oth"), ("Other".into(), "#94A3B8".into()));
    assert_eq!(
        read_type_color("weird"),
        ("Other".into(), "#94A3B8".into()),
        "unrecognized liability_type must fall back to Other, never fail the migration"
    );

    // liabilities table is gone.
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='liabilities'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(table_exists, 0);
}

#[test]
fn balance_sign_is_flipped_and_debt_fields_are_preserved() {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    pre_v039_schema(&conn);

    conn.execute(
        "INSERT INTO liabilities(id, name, liability_type, balance_cents, limit_cents, apr_pct, \
                                  min_payment_cents, payoff_date, original_balance_cents, started_at, \
                                  currency, created_at, updated_at) \
         VALUES('amex', 'Amex Card', 'credit-card', 113900, 500000, 24.9, 5000, '2027-01-01', \
                200000, '2023-05-01', 'CAD', '2024-01-01T00:00:00Z', '2024-06-01T00:00:00Z')",
        [],
    )
    .unwrap();

    run_v039(&conn);

    let (apr, min_pay, payoff, limit, orig, started, currency): (
        Option<f64>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT apr_pct, min_payment_cents, payoff_date, limit_cents, original_balance_cents, started_at, currency \
             FROM accounts WHERE id = 'amex'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .unwrap();
    assert_eq!(apr, Some(24.9));
    assert_eq!(min_pay, Some(5000));
    assert_eq!(payoff.as_deref(), Some("2027-01-01"));
    assert_eq!(limit, Some(500000));
    assert_eq!(orig, Some(200000));
    assert_eq!(started.as_deref(), Some("2023-05-01"));
    assert_eq!(currency, "CAD");

    // The liability stored the POSITIVE amount owed; the account convention
    // is negative for debt — this is the balance the double-count bug hinged on.
    let balance: i64 = conn
        .query_row(
            "SELECT balance_cents FROM account_balances WHERE account_id = 'amex'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(balance, -113_900, "liability balance must be sign-flipped onto the account");
}

#[test]
fn goal_linked_via_liability_id_is_repointed_to_account_id_and_the_column_is_dropped() {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    pre_v039_schema(&conn);

    conn.execute(
        "INSERT INTO liabilities(id, name, liability_type, balance_cents, currency, created_at, updated_at) \
         VALUES('carloan', 'Car Loan', 'loan', 1500000, 'USD', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) \
         VALUES('someacct', 'Me', 'Bank', 'Checking', 'Checking', 'USD', '#fff', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO goals(id, name, type, target_cents, liability_id, created_at) \
         VALUES('g1', 'Pay off car', 'debt-payoff', 1500000, 'carloan', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    // A goal already linked to a real account must be left alone.
    conn.execute(
        "INSERT INTO goals(id, name, type, target_cents, account_id, created_at) \
         VALUES('g2', 'Emergency fund', 'build-balance', 500000, 'someacct', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    run_v039(&conn);

    let (g1_account, ) : (Option<String>, ) = conn
        .query_row("SELECT account_id FROM goals WHERE id = 'g1'", [], |r| Ok((r.get(0)?,)))
        .unwrap();
    assert_eq!(g1_account.as_deref(), Some("carloan"), "liability_id must be copied onto account_id");

    let (g2_account, ): (Option<String>,) = conn
        .query_row("SELECT account_id FROM goals WHERE id = 'g2'", [], |r| Ok((r.get(0)?,)))
        .unwrap();
    assert_eq!(g2_account.as_deref(), Some("someacct"), "an existing account_id must not be clobbered");

    // liability_id column itself is gone.
    let mut stmt = conn.prepare("PRAGMA table_info(goals)").unwrap();
    let columns: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!columns.contains(&"liability_id".to_string()), "liability_id column must be dropped");
    assert!(columns.contains(&"account_id".to_string()), "account_id column must survive");
}
