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

fn read_v030_migration() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations")
        .join("V030__category_palette.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
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

fn run_v030_sql(conn: &Connection) {
    let sql = read_v030_migration();
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
