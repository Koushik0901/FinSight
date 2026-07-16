use crate::error::AppResult;
use chrono::{Duration, NaiveDate, Utc};
use finsight_core::repos::run;
use finsight_core::{settings, Db};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

const ENABLED_KEY: &str = "notifications.enabled";

struct EnvelopeRow {
    category_id: String,
    label: String,
    budget: i64,
    spent: i64,
}

/// Budget envelopes over their limit for `month`/`month_start` (`month_start`
/// = `"{month}-01"`). A `settle_up = 1` reimbursement inflow nets against the
/// category's spend (matching metrics.rs cashflow) instead of being silently
/// dropped by an `amount_cents < 0`-only filter — otherwise this fires a stale
/// "over budget" push notification the user already resolved. Extracted from
/// [`check_and_fire`] so it's directly unit-testable without a Tauri
/// `AppHandle`.
fn over_budget_envelopes(
    conn: &rusqlite::Connection,
    month_start: &str,
    month: &str,
) -> finsight_core::CoreResult<Vec<EnvelopeRow>> {
    let mut stmt = conn.prepare(
        "WITH spending AS (
           SELECT t.category_id,
                  CASE WHEN t.settle_up = 1 THEN -t.amount_cents ELSE ABS(t.amount_cents) END AS cents,
                  t.posted_at
           FROM transactions t
           WHERE (t.amount_cents < 0 OR t.settle_up = 1)
             AND t.category_id IS NOT NULL
             AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = t.id)
           UNION ALL
           SELECT ts.category_id,
                  CASE WHEN t.settle_up = 1 THEN -ts.amount_cents ELSE ts.amount_cents END AS cents,
                  t.posted_at
           FROM transaction_splits ts
           JOIN transactions t ON t.id = ts.txn_id
           WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND ts.category_id IS NOT NULL
         )
         SELECT c.id, c.label, b.amount_cents,
                COALESCE(SUM(CASE WHEN s.posted_at >= ?1 THEN s.cents ELSE 0 END), 0) AS spent
         FROM categories c
         JOIN budgets b ON b.category_id = c.id AND b.month = ?2
         LEFT JOIN spending s ON s.category_id = c.id
         WHERE c.archived_at IS NULL AND b.amount_cents > 0
         GROUP BY c.id, c.label, b.amount_cents
         HAVING spent > b.amount_cents",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![month_start, month], |r| {
            Ok(EnvelopeRow {
                category_id: r.get(0)?,
                label: r.get(1)?,
                budget: r.get(2)?,
                spent: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;
    Ok(rows)
}

pub async fn check_and_fire(app: &AppHandle, db: &Db) -> AppResult<()> {
    let db = db.clone();
    let app = app.clone();
    let to_fire = run(&db, move |conn| {
        // Check enabled (default true when absent)
        let enabled: Option<bool> = settings::get(conn, ENABLED_KEY)?;
        if enabled == Some(false) {
            return Ok(vec![]);
        }

        let mut notifications: Vec<(String, String)> = Vec::new(); // (title, body)
        let now = Utc::now();
        let this_month = now.format("%Y-%m").to_string();

        // ── 1. Budget overflow check ──────────────────────────────────────────
        let this_month_start = now.format("%Y-%m-01").to_string();

        let over_envelopes: Vec<EnvelopeRow> =
            over_budget_envelopes(conn, &this_month_start, &this_month)?;

        for env in &over_envelopes {
            let dedup_key = format!("notifications.overflow.{}.{}", env.category_id, this_month);
            let already_sent: Option<bool> = settings::get(conn, &dedup_key)?;
            if already_sent.is_some() {
                continue;
            }
            notifications.push((
                format!("{} · over budget", env.label),
                format!(
                    "${:.2} spent of ${:.2} budget",
                    env.spent as f64 / 100.0,
                    env.budget as f64 / 100.0
                ),
            ));
            settings::set(conn, &dedup_key, &true)?;
        }

        // ── 2. Bill due in 3 days check ───────────────────────────────────────
        let cutoff = (now - Duration::days(395)).format("%Y-%m-%d").to_string();
        let today = now.format("%Y-%m-%d").to_string();
        let in_3 = (now + Duration::days(3)).format("%Y-%m-%d").to_string();

        let mut stmt2 = conn.prepare(
            "WITH dated AS (
               SELECT merchant_raw, date(posted_at) AS d, amount_cents,
                      LAG(date(posted_at)) OVER (PARTITION BY merchant_raw ORDER BY posted_at) AS prev_d
               FROM transactions
               WHERE posted_at >= ?1
             ),
             gaps AS (
               SELECT merchant_raw, d, amount_cents,
                      julianday(d) - julianday(prev_d) AS gap
               FROM dated WHERE prev_d IS NOT NULL
             ),
             agg AS (
               SELECT merchant_raw, AVG(gap) AS avg_gap, MAX(d) AS last_seen,
                      MAX(amount_cents) AS last_amount, COUNT(*) AS occ
               FROM gaps WHERE gap BETWEEN 5 AND 400
               GROUP BY merchant_raw
               HAVING occ >= 2 AND AVG(gap) < 400 AND MAX(amount_cents) < 0
             )
             SELECT merchant_raw, avg_gap, last_seen, last_amount FROM agg",
        )?;

        struct BillCandidate {
            merchant: String,
            next_str: String,
            amount: i64,
        }
        let candidates: Vec<BillCandidate> = stmt2
            .query_map(rusqlite::params![cutoff], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(merchant, avg_gap, last_seen, amount)| {
                let last = NaiveDate::parse_from_str(&last_seen, "%Y-%m-%d").ok()?;
                let next = last + Duration::days(avg_gap.round() as i64);
                let next_str = next.format("%Y-%m-%d").to_string();
                if next_str >= today && next_str <= in_3 {
                    Some(BillCandidate {
                        merchant,
                        next_str,
                        amount,
                    })
                } else {
                    None
                }
            })
            .collect();
        // stmt2 borrow released here — conn is free again for settings reads/writes

        let today_naive = NaiveDate::parse_from_str(&today, "%Y-%m-%d").ok();
        for bill in &candidates {
            let dedup_key = format!(
                "notifications.bill.{}.{}",
                bill.merchant.to_lowercase().replace(' ', "_"),
                bill.next_str
            );
            let already: Option<bool> = settings::get(conn, &dedup_key)?;
            if already.is_some() {
                continue;
            }
            let days_away = today_naive
                .and_then(|t| {
                    NaiveDate::parse_from_str(&bill.next_str, "%Y-%m-%d")
                        .ok()
                        .map(|n| (n - t).num_days())
                })
                .unwrap_or(0);
            let when = if days_away == 0 {
                "today".to_string()
            } else {
                format!("in {days_away} day{}", if days_away == 1 { "" } else { "s" })
            };
            notifications.push((
                format!("{} · due {when}", bill.merchant),
                format!("${:.2}", (bill.amount.unsigned_abs() as f64) / 100.0),
            ));
            settings::set(conn, &dedup_key, &true)?;
        }

        Ok(notifications)
    })
    .await
    .map_err(crate::error::AppError::from)?;

    for (title, body) in to_fire {
        let _ = app
            .notification()
            .builder()
            .title(&title)
            .body(&body)
            .show();
    }
    Ok(())
}

/// Fire a single ad-hoc OS notification, honoring the user's
/// `notifications.enabled` setting (default on when unset). Best-effort: a
/// settings-read failure falls back to "enabled", and a `show()` failure is
/// swallowed — a notification is never load-bearing. Used for event-driven
/// notifications (e.g. a background Copilot "deep answer" landing) that don't
/// belong in the periodic budget/bill sweep above.
pub async fn fire_notification(app: &AppHandle, db: &Db, title: &str, body: &str) {
    let db = db.clone();
    let enabled = run(&db, |conn| {
        let v: Option<bool> = settings::get(conn, ENABLED_KEY)?;
        Ok::<_, finsight_core::CoreError>(v.unwrap_or(true))
    })
    .await
    .unwrap_or(true);
    if !enabled {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("notifications.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_account(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('a1','Me','Bank','Checking','Checking','USD','#fff',datetime('now'))",
            [],
        )
        .unwrap();
    }

    fn seed_category(conn: &rusqlite::Connection, id: &str, label: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('grp', 'Group', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'grp', ?2, '#94A3B8', 0)",
            rusqlite::params![id, label],
        )
        .unwrap();
    }

    #[test]
    fn over_budget_envelopes_nets_settle_up_inflow() {
        // A settle_up = 1 reimbursement inflow must reduce the envelope's
        // reported spend instead of being silently dropped by an
        // `amount_cents < 0`-only filter — otherwise this fires a stale "over
        // budget" push notification for spend the user already got back.
        let (_dir, db) = fresh_db();
        let conn = db.get().unwrap();
        seed_account(&conn);
        seed_category(&conn, "food", "Food");

        let month = "2026-05";
        let month_start = "2026-05-01";
        conn.execute(
            "INSERT INTO budgets(id,category_id,month,amount_cents,created_at,updated_at) \
             VALUES('b1','food',?1,4000,datetime('now'),datetime('now'))",
            rusqlite::params![month],
        )
        .unwrap();

        // Ordinary $50 grocery expense — over the $40 budget on its own.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,is_transfer,created_at) \
             VALUES('e1','a1','2026-05-10T00:00:00Z',-5000,'GROCERY','food','cleared',0,0,'2026-05-10T00:00:00Z')",
            [],
        )
        .unwrap();
        // A $20 settle-up reimbursement brings net spend to $30 — under budget.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,is_transfer,created_at,settle_up) \
             VALUES('su1','a1','2026-05-12T00:00:00Z',2000,'FRIEND REFUND','food','cleared',0,0,'2026-05-12T00:00:00Z',1)",
            [],
        )
        .unwrap();

        let over = over_budget_envelopes(&conn, month_start, month).unwrap();
        assert!(
            over.is_empty(),
            "netted spend (3000) is under the 4000 budget — must not fire an over-budget notification"
        );
    }
}
