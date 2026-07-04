use finsight_core::{db::run_migrations, palette, Db};
use rusqlite::Connection;
use tempfile::tempdir;

/// Mirrors the canonical 10 starter categories seeded by `sample.rs`,
/// `commit_starter_categories`, and the dev-demo seed path.
const CANONICAL_CATEGORIES: &[(&str, &str)] = &[
    ("groceries", "daily"),
    ("dining", "daily"),
    ("transport", "daily"),
    ("housing", "fixed"),
    ("utilities", "fixed"),
    ("subscriptions", "fixed"),
    ("shopping", "lifestyle"),
    ("travel", "lifestyle"),
    ("gifts", "lifestyle"),
    ("health", "wellbeing"),
];

fn read_migration(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn read_v030_migration() -> String {
    read_migration("V030__category_palette.sql")
}

fn open_db() -> Db {
    let dir = tempdir().unwrap();
    let path = dir.path().join("m.sqlcipher");
    let key = "ab".repeat(32);
    Db::open(&path, &key).unwrap()
}

fn seed_grey_categories(conn: &Connection) {
    for (_, group) in CANONICAL_CATEGORIES {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label) VALUES(?1, ?1)",
            rusqlite::params![group],
        )
        .unwrap();
    }
    for (id, group) in CANONICAL_CATEGORIES {
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) \
             VALUES(?1, ?2, ?1, '#94A3B8', 0)",
            rusqlite::params![id, group],
        )
        .unwrap();
    }
}

fn read_color(conn: &Connection, id: &str) -> String {
    conn.query_row(
        "SELECT color FROM categories WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )
    .unwrap()
}

fn run_migration_sql(conn: &Connection, sql: &str) {
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

fn run_v030_sql(conn: &Connection) {
    run_migration_sql(conn, &read_v030_migration());
}

fn run_v036_sql(conn: &Connection) {
    run_migration_sql(conn, &read_migration("V036__category_palette_regrey_backfill.sql"));
}

#[test]
fn v030_migration_backfills_canonical_category_colors() {
    let db = open_db();
    run_migrations(&db).unwrap();
    let conn = db.get().unwrap();

    // Simulate the buggy pre-fix state: every category stamped grey.
    seed_grey_categories(&conn);
    for (id, _) in CANONICAL_CATEGORIES {
        assert_eq!(read_color(&conn, id), "#94A3B8", "{id} starts grey");
    }

    // The migration file is idempotent — it ran as part of `run_migrations`
    // above, but at that point the categories did not exist yet, so it was a
    // no-op. Now we re-apply it to simulate an existing user DB that needs
    // the backfill.
    run_v030_sql(&conn);

    for (id, _) in CANONICAL_CATEGORIES {
        let expected = palette::color_for(id);
        assert_eq!(
            read_color(&conn, id),
            expected,
            "{id} should be backfilled to {expected}"
        );
    }
}

#[test]
fn v036_migration_fixes_regreyed_defaults_but_keeps_user_chosen_colors() {
    let db = open_db();
    run_migrations(&db).unwrap();
    let conn = db.get().unwrap();

    // Simulate the state fixed by V036: ensure_default_categories seeded grey
    // AFTER V030 already ran — except the user re-colored one category.
    seed_grey_categories(&conn);
    conn.execute(
        "UPDATE categories SET color = '#112233' WHERE id = 'dining'",
        [],
    )
    .unwrap();

    run_v036_sql(&conn);

    for (id, _) in CANONICAL_CATEGORIES {
        if *id == "dining" {
            continue;
        }
        assert_eq!(
            read_color(&conn, id),
            palette::color_for(id),
            "{id} should be backfilled to its palette color"
        );
    }
    assert_eq!(
        read_color(&conn, "dining"),
        "#112233",
        "a user-chosen color must survive the V036 backfill"
    );
}

#[test]
fn v037_migration_backfills_spending_types_but_keeps_user_tags() {
    let db = open_db();
    run_migrations(&db).unwrap();
    let conn = db.get().unwrap();

    // Categories seeded WITHOUT spending types (the pre-fix state), except the
    // user already tagged one themselves.
    seed_grey_categories(&conn);
    conn.execute(
        "UPDATE categories SET spending_type = 'savings' WHERE id = 'travel'",
        [],
    )
    .unwrap();

    run_migration_sql(
        &conn,
        &read_migration("V037__default_spending_types.sql"),
    );

    let read = |id: &str| -> Option<String> {
        conn.query_row(
            "SELECT spending_type FROM categories WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(read("housing").as_deref(), Some("fixed"));
    assert_eq!(read("dining").as_deref(), Some("guilt_free"));
    assert_eq!(
        read("travel").as_deref(),
        Some("savings"),
        "a user-chosen spending type must survive the backfill"
    );
}

#[test]
fn v030_migration_leaves_custom_categories_untouched() {
    let db = open_db();
    run_migrations(&db).unwrap();
    let conn = db.get().unwrap();

    // A user-created custom category.
    conn.execute(
        "INSERT OR IGNORE INTO category_groups(id, label) VALUES('lifestyle', 'Lifestyle')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO categories(id, group_id, label, color, sort_order) \
         VALUES('pet_care', 'lifestyle', 'Pet Care', '#112233', 0)",
        [],
    )
    .unwrap();

    run_v030_sql(&conn);

    assert_eq!(
        read_color(&conn, "pet_care"),
        "#112233",
        "custom category color must not be overwritten by the backfill"
    );
}
