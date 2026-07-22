//! Subscription-change surfacing (issue #58).
//!
//! Turns the recurring detector's per-series signals — a material price change,
//! an imminent annual renewal — into user-facing alerts through the unified
//! notification policy (#57), and holds the user's confirm/dismiss verdict on a
//! detected subscription (which derived detection has nowhere else to store).
//!
//! **One detection path.** Everything here reads
//! [`recurring::detect_recurring`] — the same source the `/recurring` screen
//! displays — so a CSV-only user and a bank-synced user get identical results.
//! Only the *trigger* differs: the display is read at screen-open time (covers
//! everyone), while notification emission is driven by a producer wired into
//! both CSV-import completion and the background sync cycle.

use crate::error::CoreResult;
use crate::notify::{self, NewNotification, NotificationCategory, Urgency};
use crate::recurring::{detect_recurring, RecurringItem};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection};
use std::collections::HashMap;

/// How far back the ALERT scan looks — deliberately longer than the 13-month
/// `/recurring` display window: a once-yearly subscription needs ~3 years of
/// history to reach `MIN_OCCURRENCES`, so annual renewals are invisible in a
/// 13-month window. The detection LOGIC is the same shared `detect_recurring`;
/// only the window differs, and the recency guards below keep a long window
/// from first-alerting on ancient changes.
const SCAN_WINDOW_DAYS: i64 = 1100;
/// Lead time before a renewal at which to notify.
const RENEWAL_LEAD_DAYS: i64 = 14;
/// Only long-cadence (annual-ish) renewals are worth a heads-up — a monthly
/// renewal is not a surprise worth a notification.
const RENEWAL_MIN_GAP_DAYS: f64 = 300.0;
/// A price change older than this is stale — the user has already seen charges
/// at the new price, so a long-window scan must not first-alert on it. Also the
/// TTL after which an un-actioned price alert lapses.
const PRICE_CHANGE_TTL_DAYS: i64 = 90;

/// The user's durable decision about a detected subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionVerdict {
    /// This is a real subscription — keep surfacing it.
    Confirmed,
    /// Not a subscription (or don't alert me) — suppress its alerts.
    Dismissed,
}

impl SubscriptionVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Dismissed => "dismissed",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "confirmed" => Some(Self::Confirmed),
            "dismissed" => Some(Self::Dismissed),
            _ => None,
        }
    }
}

/// Set (or clear, with `None`) the user's verdict on a detected subscription,
/// keyed by the same canonical merchant key the detector groups on.
pub fn set_verdict(
    conn: &Connection,
    merchant_key: &str,
    verdict: Option<SubscriptionVerdict>,
) -> CoreResult<()> {
    match verdict {
        Some(v) => {
            conn.execute(
                "INSERT INTO subscription_overrides(merchant_key, verdict, created_at) VALUES(?1,?2,?3) \
                 ON CONFLICT(merchant_key) DO UPDATE SET verdict=excluded.verdict, created_at=excluded.created_at",
                params![merchant_key, v.as_str(), Utc::now().to_rfc3339()],
            )?;
        }
        None => {
            conn.execute(
                "DELETE FROM subscription_overrides WHERE merchant_key = ?1",
                params![merchant_key],
            )?;
        }
    }
    Ok(())
}

/// All stored verdicts, keyed by merchant_key.
pub fn load_verdicts(conn: &Connection) -> CoreResult<HashMap<String, SubscriptionVerdict>> {
    let mut stmt = conn.prepare("SELECT merchant_key, verdict FROM subscription_overrides")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = HashMap::new();
    for row in rows {
        let (k, v) = row?;
        if let Some(verdict) = SubscriptionVerdict::from_str(&v) {
            out.insert(k, verdict);
        }
    }
    Ok(out)
}

/// Producer: scan detected subscriptions and enqueue notifications for material
/// price changes and imminent annual renewals through the shared policy.
///
/// Idempotent — a standing change/renewal dedups to a single alert via a stable
/// per-instance key, so running this on every import and sync cycle is safe.
/// Series the user dismissed, and detections too weak to be treated as real
/// obligations, are skipped. Returns how many alerts were newly delivered
/// (i.e. eligible to push now).
pub fn refresh_subscription_alerts(conn: &mut Connection, now: DateTime<Utc>) -> CoreResult<usize> {
    let items = detect_recurring(conn, SCAN_WINDOW_DAYS)?;
    let verdicts = load_verdicts(conn)?;
    let prefs = notify::load_prefs(conn);
    let today = now.date_naive();
    let mut delivered = 0usize;

    for item in &items {
        if matches!(
            verdicts.get(&item.merchant_key),
            Some(SubscriptionVerdict::Dismissed)
        ) {
            continue;
        }
        // Only alert on costs the app is confident enough to treat as real
        // recurring obligations — display is not gated, but a push is.
        if !item.is_projection_obligation() {
            continue;
        }

        if let Some(alert) = price_change_alert(item, today) {
            if notify::enqueue(conn, alert, &prefs, now)?.push {
                delivered += 1;
            }
        }
        if let Some(alert) = renewal_alert(item, today) {
            if notify::enqueue(conn, alert, &prefs, now)?.push {
                delivered += 1;
            }
        }
    }
    Ok(delivered)
}

/// Build the notification for a detected price change, or `None` if the item
/// has none. The merchant and figures live in `sensitive` (redacted from push
/// under the user's privacy setting); `body` stands alone.
fn price_change_alert(item: &RecurringItem, today: NaiveDate) -> Option<NewNotification> {
    let pc = item.price_change.as_ref()?;
    // Don't first-alert a change the user has long since lived with — the long
    // scan window would otherwise surface a price step from years ago.
    let effective = parse_date(&pc.effective_date)?;
    if (today - effective).num_days() > PRICE_CHANGE_TTL_DAYS {
        return None;
    }
    let up = pc.to_cents >= pc.from_cents;
    let sign = if pc.pct >= 0.0 { "+" } else { "" };
    Some(NewNotification {
        category: NotificationCategory::SubscriptionChange,
        urgency: Urgency::Normal,
        dedup_key: format!("sub.price.{}.{}", item.merchant_key, pc.effective_date),
        title: if up {
            "A subscription price went up".into()
        } else {
            "A subscription price went down".into()
        },
        body: "A recurring charge changed price.".into(),
        sensitive: Some(format!(
            "{}: {} → {} ({sign}{:.1}%)",
            item.display_merchant,
            fmt_money(pc.from_cents, &pc.currency),
            fmt_money(pc.to_cents, &pc.currency),
            pc.pct,
        )),
        route: Some("/recurring".into()),
        expires_at: Some(midnight_rfc3339(effective + Duration::days(PRICE_CHANGE_TTL_DAYS))),
    })
}

/// Build the notification for an imminent annual renewal, or `None`.
fn renewal_alert(item: &RecurringItem, today: NaiveDate) -> Option<NewNotification> {
    if item.avg_gap_days < RENEWAL_MIN_GAP_DAYS {
        return None;
    }
    let next = item.next_expected.as_deref().and_then(parse_date)?;
    let days = (next - today).num_days();
    if !(0..=RENEWAL_LEAD_DAYS).contains(&days) {
        return None;
    }
    let currency = item
        .price_change
        .as_ref()
        .map(|pc| pc.currency.clone())
        .unwrap_or_default();
    Some(NewNotification {
        category: NotificationCategory::SubscriptionChange,
        urgency: Urgency::Normal,
        dedup_key: format!("sub.renewal.{}.{}", item.merchant_key, next.format("%Y-%m-%d")),
        title: "A subscription renews soon".into(),
        body: format!("An annual subscription renews within {RENEWAL_LEAD_DAYS} days."),
        sensitive: Some(format!(
            "{}: {} on {}",
            item.display_merchant,
            fmt_money(item.last_amount_cents, &currency),
            next.format("%Y-%m-%d"),
        )),
        route: Some("/recurring".into()),
        expires_at: Some(midnight_rfc3339(next + Duration::days(3))),
    })
}

/// Format cents in the series' currency, never assuming a symbol — an empty
/// currency prints the bare number.
fn fmt_money(cents: i64, currency: &str) -> String {
    let v = (cents.unsigned_abs() as f64) / 100.0;
    if currency.is_empty() {
        format!("{v:.2}")
    } else {
        format!("{currency} {v:.2}")
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn midnight_rfc3339(d: NaiveDate) -> String {
    d.and_hms_opt(0, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("sub.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed(conn: &Connection) {
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('a','me','B','Credit','Card','USD','#fff',datetime('now'))", []).unwrap();
    }

    fn charge(conn: &Connection, merchant: &str, date: &str, cents: i64) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
             VALUES(hex(randomblob(16)),'a',?1,?2,?3,0,'cleared',datetime('now'))",
            params![format!("{date}T12:00:00Z"), cents, merchant],
        )
        .unwrap();
    }

    /// n monthly charges from `start` at a fixed amount.
    fn monthly(conn: &Connection, merchant: &str, start: &str, n: i64, cents: i64) {
        let start = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
        for i in 0..n {
            let d = start + Duration::days(30 * i);
            charge(conn, merchant, &d.format("%Y-%m-%d").to_string(), cents);
        }
    }

    #[test]
    fn verdict_round_trips_and_clears() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        set_verdict(&conn, "spotify", Some(SubscriptionVerdict::Dismissed)).unwrap();
        assert_eq!(load_verdicts(&conn).unwrap().get("spotify"), Some(&SubscriptionVerdict::Dismissed));
        // Upsert to a different verdict.
        set_verdict(&conn, "spotify", Some(SubscriptionVerdict::Confirmed)).unwrap();
        assert_eq!(load_verdicts(&conn).unwrap().get("spotify"), Some(&SubscriptionVerdict::Confirmed));
        // Clear.
        set_verdict(&conn, "spotify", None).unwrap();
        assert!(load_verdicts(&conn).unwrap().is_empty());
    }

    /// A real price step produces exactly one alert, and a second scan of the
    /// same unchanged condition does not re-notify (dedup by effective date).
    #[test]
    fn price_step_notifies_once() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&conn);
        monthly(&conn, "SPOTIFY", "2025-01-05", 6, -999);
        for i in 0..3 {
            let d = NaiveDate::parse_from_str("2025-07-05", "%Y-%m-%d").unwrap() + Duration::days(30 * i);
            charge(&conn, "SPOTIFY", &d.format("%Y-%m-%d").to_string(), -1299);
        }
        let now = "2025-09-20T12:00:00Z".parse().unwrap();
        let n = refresh_subscription_alerts(&mut conn, now).unwrap();
        assert_eq!(n, 1, "one price-change alert delivered");

        // include_resolved=true so the assertion doesn't depend on the active
        // view's wall-clock expiry filter (this fixture uses synthetic dates).
        let all = notify::list(&mut conn, true, 50).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].category, NotificationCategory::SubscriptionChange);
        // The figures ride in `sensitive`, not the always-visible body.
        assert!(all[0].sensitive.as_deref().unwrap().contains("9.99"));
        assert!(all[0].sensitive.as_deref().unwrap().contains("12.99"));
        assert!(!all[0].body.contains("12.99"), "amounts must not be in the push-visible body");

        // Second scan: same condition, no duplicate.
        let n2 = refresh_subscription_alerts(&mut conn, now).unwrap();
        assert_eq!(n2, 0, "no re-notify for an unchanged price");
        assert_eq!(notify::list(&mut conn, true, 50).unwrap().len(), 1);
    }

    #[test]
    fn dismissed_subscription_is_not_alerted() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&conn);
        monthly(&conn, "SPOTIFY", "2025-01-05", 6, -999);
        for i in 0..3 {
            let d = NaiveDate::parse_from_str("2025-07-05", "%Y-%m-%d").unwrap() + Duration::days(30 * i);
            charge(&conn, "SPOTIFY", &d.format("%Y-%m-%d").to_string(), -1299);
        }
        set_verdict(&conn, "spotify", Some(SubscriptionVerdict::Dismissed)).unwrap();
        let now = "2025-09-20T12:00:00Z".parse().unwrap();
        assert_eq!(refresh_subscription_alerts(&mut conn, now).unwrap(), 0, "dismissed series is silent");
        assert_eq!(notify::list(&mut conn, false, 50).unwrap().len(), 0);
    }

    #[test]
    fn imminent_annual_renewal_is_surfaced_once() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&conn);
        // A yearly renewal (~365-day gaps) whose next charge is ~10 days out.
        // Last charge dated so next_expected lands inside the 14-day lead window.
        for i in 0..3 {
            let d = NaiveDate::parse_from_str("2023-11-20", "%Y-%m-%d").unwrap() + Duration::days(365 * i);
            charge(&conn, "AMAZON PRIME MEMBERSHIP", &d.format("%Y-%m-%d").to_string(), -13900);
        }
        // now is ~10 days before the 4th-year renewal (2026-11-18 ≈ last 2025-11-19 + 365).
        let now: DateTime<Utc> = "2026-11-09T12:00:00Z".parse().unwrap();
        let n = refresh_subscription_alerts(&mut conn, now).unwrap();
        assert_eq!(n, 1, "an imminent annual renewal is surfaced");
        // include_resolved=true: robust to the active view's wall-clock expiry.
        let all = notify::list(&mut conn, true, 50).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].sensitive.as_deref().unwrap().contains("139.00"));

        // Far from the window: no renewal alert. (Fresh DB, charge far in past.)
        let (_d2, db2) = fresh();
        let mut conn2 = db2.get().unwrap();
        seed(&conn2);
        for i in 0..3 {
            let d = NaiveDate::parse_from_str("2023-11-20", "%Y-%m-%d").unwrap() + Duration::days(365 * i);
            charge(&conn2, "AMAZON PRIME MEMBERSHIP", &d.format("%Y-%m-%d").to_string(), -13900);
        }
        let far: DateTime<Utc> = "2026-06-01T12:00:00Z".parse().unwrap();
        assert_eq!(refresh_subscription_alerts(&mut conn2, far).unwrap(), 0, "renewal months away is not surfaced");
    }

    #[test]
    fn steady_subscription_produces_no_alert() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&conn);
        monthly(&conn, "NETFLIX", "2025-01-04", 10, -1599);
        let now = "2025-11-01T12:00:00Z".parse().unwrap();
        assert_eq!(refresh_subscription_alerts(&mut conn, now).unwrap(), 0);
        assert_eq!(notify::list(&mut conn, false, 50).unwrap().len(), 0);
    }
}
