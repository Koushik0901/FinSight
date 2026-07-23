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
use crate::recurring::{detect_recurring, RecurringItem, RecurringKind};
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
/// TTL after which an un-actioned price alert lapses. Reused as the recency
/// bound for a post-cancellation charge (#75).
const PRICE_CHANGE_TTL_DAYS: i64 = 90;
/// Lead time before a recorded trial's end date at which to warn it converts.
const TRIAL_LEAD_DAYS: i64 = 3;
/// Two subscriptions both charged within this many days count as "both active"
/// for duplicate detection — an old, lapsed series isn't a live double-charge.
const DUPLICATE_RECENT_DAYS: i64 = 45;
/// Duplicate candidates must charge within this percent of each other; two
/// same-named services at very different prices are likely genuinely different.
const DUPLICATE_AMOUNT_TOLERANCE_PCT: i64 = 10;
/// A duplicate suggestion lapses after this long if un-actioned — the backstop
/// that clears a stale pairing once one series lapses (its keyed dedup would
/// otherwise linger, since the group's exact membership is in the dedup key). A
/// still-valid duplicate simply re-surfaces on the next cycle after it expires.
const DUPLICATE_TTL_DAYS: i64 = 60;

/// The user's durable decision about a detected subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionVerdict {
    /// This is a real subscription — keep surfacing it.
    Confirmed,
    /// Not a subscription (or don't alert me) — suppress its alerts.
    Dismissed,
    /// A real subscription the user has ENDED — distinct from dismissed. Its
    /// ongoing price/renewal alerts stop, but a charge dated after the cancel
    /// date is surfaced ("you thought this was cancelled"). (#75)
    Cancelled,
}

impl SubscriptionVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Dismissed => "dismissed",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "confirmed" => Some(Self::Confirmed),
            "dismissed" => Some(Self::Dismissed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// The full lifecycle record a user has attached to a detected subscription: the
/// verdict plus the #75 facts — a captured display `label`, a `trial_ends_at`
/// (this is a free trial converting on that date), and a `cancelled_at` (the
/// boundary after which a charge is a surprise). Any subset may be present.
#[derive(Debug, Clone)]
pub struct SubscriptionOverride {
    pub verdict: SubscriptionVerdict,
    pub label: Option<String>,
    pub trial_ends_at: Option<String>,
    pub cancelled_at: Option<String>,
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

/// All stored lifecycle overrides, keyed by merchant_key.
pub fn load_overrides(conn: &Connection) -> CoreResult<HashMap<String, SubscriptionOverride>> {
    let mut stmt = conn.prepare(
        "SELECT merchant_key, verdict, label, trial_ends_at, cancelled_at FROM subscription_overrides",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (k, v, label, trial_ends_at, cancelled_at) = row?;
        if let Some(verdict) = SubscriptionVerdict::from_str(&v) {
            out.insert(k, SubscriptionOverride { verdict, label, trial_ends_at, cancelled_at });
        }
    }
    Ok(out)
}

/// All stored verdicts, keyed by merchant_key (a projection of [`load_overrides`]).
pub fn load_verdicts(conn: &Connection) -> CoreResult<HashMap<String, SubscriptionVerdict>> {
    Ok(load_overrides(conn)?.into_iter().map(|(k, o)| (k, o.verdict)).collect())
}

/// Mark a detected subscription as a free TRIAL that converts on `trial_ends_at`
/// (a `YYYY-MM-DD` date), or clear the trial with `None`. `label` is the display
/// name captured now, so the reminder can name the service even if the series is
/// later too sparse to re-detect. Marking a trial affirms it's a subscription,
/// so a brand-new row is created `confirmed`; an existing verdict is preserved.
pub fn set_subscription_trial(
    conn: &Connection,
    merchant_key: &str,
    label: &str,
    trial_ends_at: Option<&str>,
) -> CoreResult<()> {
    match trial_ends_at {
        // Marking a trial affirms this IS a subscription, so force the verdict to
        // `confirmed` even on an existing row — otherwise a dismissed/cancelled
        // series would show a trial badge whose reminder could never fire.
        Some(ends) => {
            conn.execute(
                "INSERT INTO subscription_overrides(merchant_key, verdict, label, trial_ends_at, created_at) \
                 VALUES(?1, 'confirmed', ?2, ?3, ?4) \
                 ON CONFLICT(merchant_key) DO UPDATE SET verdict='confirmed', trial_ends_at=excluded.trial_ends_at, label=excluded.label",
                params![merchant_key, label, ends, Utc::now().to_rfc3339()],
            )?;
        }
        // Clearing a trial only nulls that column; the verdict is left untouched
        // (and a row that never existed needs nothing cleared).
        None => {
            conn.execute(
                "UPDATE subscription_overrides SET trial_ends_at = NULL WHERE merchant_key = ?1",
                params![merchant_key],
            )?;
        }
    }
    Ok(())
}

/// Mark a detected subscription as CANCELLED as of `cancelled_at` (a `YYYY-MM-DD`
/// date). Sets the `cancelled` verdict (distinct from `dismissed`): ongoing
/// price/renewal alerts stop, but a charge dated after `cancelled_at` is
/// surfaced as a surprise. `label` names the service in that alert.
pub fn mark_subscription_cancelled(
    conn: &Connection,
    merchant_key: &str,
    label: &str,
    cancelled_at: &str,
) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO subscription_overrides(merchant_key, verdict, label, cancelled_at, created_at) \
         VALUES(?1, 'cancelled', ?2, ?3, ?4) \
         ON CONFLICT(merchant_key) DO UPDATE SET verdict='cancelled', cancelled_at=excluded.cancelled_at, label=excluded.label",
        params![merchant_key, label, cancelled_at, Utc::now().to_rfc3339()],
    )?;
    Ok(())
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
    let overrides = load_overrides(conn)?;
    let prefs = notify::load_prefs(conn);
    let today = now.date_naive();
    let mut delivered = 0usize;

    for item in &items {
        let ov = overrides.get(&item.merchant_key);
        match ov.map(|o| o.verdict) {
            // Dismissed = "not a subscription / don't alert" — fully silent.
            Some(SubscriptionVerdict::Dismissed) => continue,
            // Cancelled = a real subscription the user ended: no ongoing
            // price/renewal alerts, but surface a charge after the cancel date.
            Some(SubscriptionVerdict::Cancelled) => {
                if let Some(alert) = ov.and_then(|o| post_cancellation_alert(item, o, today)) {
                    if notify::enqueue(conn, alert, &prefs, now)?.push {
                        delivered += 1;
                    }
                }
                continue;
            }
            _ => {}
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

    // Trials are user-scheduled reminders keyed off recorded overrides, not the
    // detector — a trial has too few charges yet to be a detected series.
    for (key, ov) in &overrides {
        if let Some(alert) = trial_conversion_alert(key, ov, today) {
            if notify::enqueue(conn, alert, &prefs, now)?.push {
                delivered += 1;
            }
        }
    }

    // Duplicate subscriptions: two active series that look like the same service.
    for alert in duplicate_alerts(&items, &overrides, today) {
        if notify::enqueue(conn, alert, &prefs, now)?.push {
            delivered += 1;
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

/// Build the reminder for a recorded free trial about to convert, or `None`.
/// Driven by the user's override (not the detector) because a trial is too new
/// to be a detected series. Only an active (confirmed) trial converts.
fn trial_conversion_alert(
    merchant_key: &str,
    ov: &SubscriptionOverride,
    today: NaiveDate,
) -> Option<NewNotification> {
    if ov.verdict != SubscriptionVerdict::Confirmed {
        return None;
    }
    let ends = ov.trial_ends_at.as_deref().and_then(parse_date)?;
    let days = (ends - today).num_days();
    if !(0..=TRIAL_LEAD_DAYS).contains(&days) {
        return None;
    }
    let label = ov.label.clone().filter(|s| !s.is_empty());
    Some(NewNotification {
        category: NotificationCategory::SubscriptionChange,
        urgency: Urgency::Normal,
        dedup_key: format!("sub.trial.{}.{}", merchant_key, ends.format("%Y-%m-%d")),
        title: "A free trial is about to convert".into(),
        body: format!("A free trial ends within {TRIAL_LEAD_DAYS} days and will start charging you."),
        sensitive: Some(match label {
            Some(l) => format!("{l}: free trial ends {}", ends.format("%Y-%m-%d")),
            None => format!("A free trial ends {}", ends.format("%Y-%m-%d")),
        }),
        route: Some("/recurring".into()),
        expires_at: Some(midnight_rfc3339(ends + Duration::days(1))),
    })
}

/// Build the alert for a charge that landed AFTER the user marked a subscription
/// cancelled, or `None`. Uses the detected series' most recent charge date; a
/// long scan window must not first-alert an ancient surprise.
fn post_cancellation_alert(
    item: &RecurringItem,
    ov: &SubscriptionOverride,
    today: NaiveDate,
) -> Option<NewNotification> {
    let cancelled = ov.cancelled_at.as_deref().and_then(parse_date)?;
    let last = parse_date(&item.last_seen)?;
    if last <= cancelled {
        return None; // nothing charged since the cancel date
    }
    if (today - last).num_days() > PRICE_CHANGE_TTL_DAYS {
        return None;
    }
    let currency = item
        .price_change
        .as_ref()
        .map(|pc| pc.currency.clone())
        .unwrap_or_default();
    let label = ov
        .label
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| item.display_merchant.clone());
    Some(NewNotification {
        category: NotificationCategory::SubscriptionChange,
        urgency: Urgency::Normal,
        dedup_key: format!("sub.postcancel.{}.{}", item.merchant_key, last.format("%Y-%m-%d")),
        title: "A cancelled subscription charged again".into(),
        body: "A subscription you marked cancelled has a new charge.".into(),
        sensitive: Some(format!(
            "{label}: {} on {} — after you cancelled on {}",
            fmt_money(item.last_amount_cents, &currency),
            last.format("%Y-%m-%d"),
            cancelled.format("%Y-%m-%d"),
        )),
        route: Some("/recurring".into()),
        expires_at: Some(midnight_rfc3339(last + Duration::days(PRICE_CHANGE_TTL_DAYS))),
    })
}

/// A conservative "same service" key: the FULL set of ≥4-char alphabetic tokens
/// in a merchant descriptor, lowercased, deduped and sorted. `None` when there's
/// no strong token. Using the whole token set (not just the longest run) is what
/// keeps same-brand, different-service subscriptions apart — "APPLE MUSIC"
/// → `apple|music` vs "APPLE TV" → `apple` don't collide — while genuine
/// variants of one service still match ("NETFLIX" and "NETFLIX.COM" both →
/// `netflix`, since "com" is too short to count). Biased toward NOT flagging:
/// duplicate detection is a review suggestion, not an assertion.
fn service_key(display: &str) -> Option<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, tokens: &mut Vec<String>| {
        if cur.len() >= 4 && !tokens.iter().any(|t| t == cur) {
            tokens.push(cur.clone());
        }
        cur.clear();
    };
    for ch in display.chars() {
        if ch.is_ascii_alphabetic() {
            cur.push(ch.to_ascii_lowercase());
        } else {
            flush(&mut cur, &mut tokens);
        }
    }
    flush(&mut cur, &mut tokens);
    if tokens.is_empty() {
        return None;
    }
    tokens.sort();
    Some(tokens.join("|"))
}

/// Surface pairs/groups of ACTIVE subscriptions that look like the same service
/// — a possible double-charge worth a review. Conservative by construction:
/// only genuine subscriptions, only recently-active ones, grouped by a strong
/// shared token, and only when their amounts are close (very different prices
/// ⇒ probably different services). One standing alert per group.
fn duplicate_alerts(
    items: &[RecurringItem],
    overrides: &HashMap<String, SubscriptionOverride>,
    today: NaiveDate,
) -> Vec<NewNotification> {
    let mut groups: HashMap<String, Vec<&RecurringItem>> = HashMap::new();
    for it in items {
        if it.kind != RecurringKind::Subscription {
            continue;
        }
        if matches!(
            overrides.get(&it.merchant_key).map(|o| o.verdict),
            Some(SubscriptionVerdict::Dismissed) | Some(SubscriptionVerdict::Cancelled)
        ) {
            continue;
        }
        // Both must be currently active — an old lapsed series isn't a live double-charge.
        let active = parse_date(&it.last_seen)
            .map_or(false, |d| d <= today && (today - d).num_days() <= DUPLICATE_RECENT_DAYS);
        if !active {
            continue;
        }
        if let Some(key) = service_key(&it.display_merchant) {
            groups.entry(key).or_default().push(it);
        }
    }

    let mut out = Vec::new();
    for group in groups.into_values() {
        if group.len() < 2 {
            continue;
        }
        // Amount guard: only flag when the charges are close.
        let amounts: Vec<i64> = group.iter().map(|it| it.last_amount_cents.abs()).collect();
        let (min, max) = (
            *amounts.iter().min().unwrap_or(&0),
            *amounts.iter().max().unwrap_or(&0),
        );
        if max == 0 || (max - min) * 100 / max > DUPLICATE_AMOUNT_TOLERANCE_PCT {
            continue;
        }
        let mut keys: Vec<&str> = group.iter().map(|it| it.merchant_key.as_str()).collect();
        keys.sort_unstable();
        let names: Vec<String> = group.iter().map(|it| it.display_merchant.clone()).collect();
        out.push(NewNotification {
            category: NotificationCategory::SubscriptionChange,
            urgency: Urgency::Normal,
            dedup_key: format!("sub.dupe.{}", keys.join("+")),
            title: "Two subscriptions look like the same service".into(),
            body: "You may be paying for one service twice — worth a check.".into(),
            sensitive: Some(format!("Possible duplicate: {}", names.join(" & "))),
            // A backstop expiry so a stale pairing (one series lapses) clears
            // itself; a duplicate that still holds re-surfaces next cycle.
            expires_at: Some(midnight_rfc3339(today + Duration::days(DUPLICATE_TTL_DAYS))),
            route: Some("/recurring".into()),
        });
    }
    // Deterministic order for stable delivery/testing.
    out.sort_by(|a, b| a.dedup_key.cmp(&b.dedup_key));
    out
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

    // ── #75: trials, post-cancellation, duplicates ──────────────────────────

    /// A minimal RecurringItem for the pure duplicate/token tests, so they don't
    /// depend on the detector's classification of synthetic descriptors.
    fn ri(key: &str, display: &str, kind: RecurringKind, amount: i64, last_seen: &str) -> RecurringItem {
        RecurringItem {
            merchant_key: key.into(),
            display_merchant: display.into(),
            kind,
            confidence: 0.9,
            reasons: vec![],
            occurrences: 6,
            median_amount_cents: amount,
            last_amount_cents: amount,
            min_amount_cents: amount,
            max_amount_cents: amount,
            avg_gap_days: 30.0,
            cadence_regularity: 0.9,
            amount_stability: 0.9,
            cadence: "monthly".into(),
            category_label: None,
            category_color: None,
            last_seen: last_seen.into(),
            next_expected: None,
            price_change: None,
        }
    }

    #[test]
    fn service_key_uses_the_full_token_set() {
        // Genuine variants of one service still match ("com" is too short to count).
        assert_eq!(service_key("NETFLIX.COM"), Some("netflix".into()));
        assert_eq!(service_key("NETFLIX"), Some("netflix".into()));
        // Same brand, different services → different keys, so they never collide.
        assert_eq!(service_key("APPLE MUSIC"), Some("apple|music".into()));
        assert_ne!(service_key("APPLE MUSIC"), service_key("APPLE TV"));
        assert_eq!(service_key("SP"), None); // nothing >= 4 chars
        assert_eq!(service_key("A1 B2 99"), None); // no alphabetic run >= 4
    }

    #[test]
    fn same_brand_different_services_are_not_flagged_as_duplicates() {
        let today = NaiveDate::parse_from_str("2025-07-10", "%Y-%m-%d").unwrap();
        // Same brand, identical price, genuinely distinct services — the classic
        // false-positive the full-token-set key defeats.
        let items = vec![
            ri("applemusic", "APPLE MUSIC", RecurringKind::Subscription, -1099, "2025-07-01"),
            ri("appletv", "APPLE TV", RecurringKind::Subscription, -1099, "2025-06-28"),
            ri("applearcade", "APPLE ARCADE", RecurringKind::Subscription, -1099, "2025-06-30"),
        ];
        assert!(duplicate_alerts(&items, &HashMap::new(), today).is_empty());
    }

    #[test]
    fn marking_a_trial_reconfirms_a_dismissed_series_so_the_reminder_fires() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        set_verdict(&conn, "acme-vpn", Some(SubscriptionVerdict::Dismissed)).unwrap();
        set_subscription_trial(&conn, "acme-vpn", "Acme VPN", Some("2025-07-12")).unwrap();
        // Marking a trial re-affirms it as a subscription, so the badge and the
        // reminder stay consistent.
        assert_eq!(load_overrides(&conn).unwrap().get("acme-vpn").unwrap().verdict, SubscriptionVerdict::Confirmed);
        let now = "2025-07-10T12:00:00Z".parse().unwrap();
        assert_eq!(refresh_subscription_alerts(&mut conn, now).unwrap(), 1);

        // Clearing the trial nulls the date but leaves the verdict.
        set_subscription_trial(&conn, "acme-vpn", "Acme VPN", None).unwrap();
        let ov = load_overrides(&conn).unwrap();
        assert!(ov.get("acme-vpn").unwrap().trial_ends_at.is_none());
        assert_eq!(ov.get("acme-vpn").unwrap().verdict, SubscriptionVerdict::Confirmed);
    }

    #[test]
    fn duplicate_subscriptions_flag_same_service_similar_price() {
        let today = NaiveDate::parse_from_str("2025-07-10", "%Y-%m-%d").unwrap();
        let items = vec![
            ri("netflix", "NETFLIX", RecurringKind::Subscription, -1599, "2025-07-01"),
            ri("netflixcom", "NETFLIX.COM", RecurringKind::Subscription, -1599, "2025-06-28"),
            ri("spotify", "SPOTIFY", RecurringKind::Subscription, -999, "2025-07-02"),
        ];
        let alerts = duplicate_alerts(&items, &HashMap::new(), today);
        assert_eq!(alerts.len(), 1, "the two Netflix series flag; Spotify is alone");
        assert!(alerts[0].sensitive.as_deref().unwrap().contains("NETFLIX"));
    }

    #[test]
    fn duplicate_detection_respects_price_activity_and_verdict_guards() {
        let today = NaiveDate::parse_from_str("2025-07-10", "%Y-%m-%d").unwrap();
        // Same token, very different prices → likely different services, no flag.
        let diff_price = vec![
            ri("netflix", "NETFLIX", RecurringKind::Subscription, -1599, "2025-07-01"),
            ri("netflixann", "NETFLIX ANNUAL", RecurringKind::Subscription, -19999, "2025-06-28"),
        ];
        assert!(duplicate_alerts(&diff_price, &HashMap::new(), today).is_empty());

        // One series lapsed months ago → not an active double-charge.
        let lapsed = vec![
            ri("netflix", "NETFLIX", RecurringKind::Subscription, -1599, "2025-07-01"),
            ri("netflixcom", "NETFLIX.COM", RecurringKind::Subscription, -1599, "2025-01-01"),
        ];
        assert!(duplicate_alerts(&lapsed, &HashMap::new(), today).is_empty());

        // A dismissed series is excluded from duplicate consideration.
        let mut ov = HashMap::new();
        ov.insert(
            "netflixcom".to_string(),
            SubscriptionOverride { verdict: SubscriptionVerdict::Dismissed, label: None, trial_ends_at: None, cancelled_at: None },
        );
        let dismissed = vec![
            ri("netflix", "NETFLIX", RecurringKind::Subscription, -1599, "2025-07-01"),
            ri("netflixcom", "NETFLIX.COM", RecurringKind::Subscription, -1599, "2025-06-28"),
        ];
        assert!(duplicate_alerts(&dismissed, &ov, today).is_empty());

        // Non-subscription recurring items (a variable bill) are never dupes.
        let bills = vec![
            ri("hydro", "HYDRO ONE", RecurringKind::Bill, -8000, "2025-07-01"),
            ri("hydrox", "HYDRO ONE EAST", RecurringKind::Bill, -8000, "2025-06-28"),
        ];
        assert!(duplicate_alerts(&bills, &HashMap::new(), today).is_empty());
    }

    #[test]
    fn recorded_trial_warns_once_before_it_converts() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        // No detected series needed — a trial is user-recorded, keyed directly.
        set_subscription_trial(&conn, "acme-vpn", "Acme VPN", Some("2025-07-12")).unwrap();
        let now = "2025-07-10T12:00:00Z".parse().unwrap();
        assert_eq!(refresh_subscription_alerts(&mut conn, now).unwrap(), 1, "warns 2 days before conversion");
        let all = notify::list(&mut conn, true, 50).unwrap();
        assert!(all.iter().any(|n| n.sensitive.as_deref().unwrap_or("").contains("Acme VPN")));
        // Idempotent for the same trial end date.
        assert_eq!(refresh_subscription_alerts(&mut conn, now).unwrap(), 0);

        // A trial still far off does not warn.
        let (_d2, db2) = fresh();
        let mut conn2 = db2.get().unwrap();
        set_subscription_trial(&conn2, "acme-vpn", "Acme VPN", Some("2025-09-30")).unwrap();
        assert_eq!(refresh_subscription_alerts(&mut conn2, now).unwrap(), 0);
    }

    #[test]
    fn charge_after_cancellation_is_surfaced_but_not_before() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&conn);
        monthly(&conn, "NETFLIX", "2025-01-05", 6, -1599); // Jan..Jun
        charge(&conn, "NETFLIX", "2025-07-05", -1599); // the surprise charge
        let key = detect_recurring(&conn, 1100)
            .unwrap()
            .into_iter()
            .find(|i| i.display_merchant.to_uppercase().contains("NETFLIX"))
            .map(|i| i.merchant_key)
            .expect("netflix series detected");
        mark_subscription_cancelled(&conn, &key, "Netflix", "2025-06-20").unwrap();
        let now = "2025-07-08T12:00:00Z".parse().unwrap();
        assert_eq!(refresh_subscription_alerts(&mut conn, now).unwrap(), 1);
        let all = notify::list(&mut conn, true, 50).unwrap();
        assert!(all.iter().any(|n| n.title.contains("charged again")));

        // No charge after the cancel date → nothing surfaced.
        let (_d2, db2) = fresh();
        let mut conn2 = db2.get().unwrap();
        seed(&conn2);
        monthly(&conn2, "NETFLIX", "2025-01-05", 6, -1599); // last charge ~2025-06-04
        let key2 = detect_recurring(&conn2, 1100)
            .unwrap()
            .into_iter()
            .find(|i| i.display_merchant.to_uppercase().contains("NETFLIX"))
            .map(|i| i.merchant_key)
            .unwrap();
        mark_subscription_cancelled(&conn2, &key2, "Netflix", "2025-06-20").unwrap(); // after the last charge
        assert_eq!(refresh_subscription_alerts(&mut conn2, now).unwrap(), 0);
    }
}
