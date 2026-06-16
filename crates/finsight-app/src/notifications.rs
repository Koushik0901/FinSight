use crate::error::AppResult;
use chrono::{Duration, NaiveDate, Utc};
use finsight_core::{settings, Db};
use finsight_core::repos::run;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

const ENABLED_KEY: &str = "notifications.enabled";

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

        struct EnvelopeRow {
            category_id: String,
            label: String,
            budget: i64,
            spent: i64,
        }

        let mut stmt = conn.prepare(
            "WITH spending AS (
               SELECT t.category_id, ABS(t.amount_cents) AS cents, t.posted_at
               FROM transactions t
               WHERE t.amount_cents < 0
                 AND t.category_id IS NOT NULL
                 AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = t.id)
               UNION ALL
               SELECT ts.category_id, ts.amount_cents AS cents, t.posted_at
               FROM transaction_splits ts
               JOIN transactions t ON t.id = ts.txn_id
               WHERE t.amount_cents < 0 AND ts.category_id IS NOT NULL
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
        let over_envelopes: Vec<EnvelopeRow> = stmt
            .query_map(rusqlite::params![this_month_start, this_month], |r| {
                Ok(EnvelopeRow {
                    category_id: r.get(0)?,
                    label: r.get(1)?,
                    budget: r.get(2)?,
                    spent: r.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

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
