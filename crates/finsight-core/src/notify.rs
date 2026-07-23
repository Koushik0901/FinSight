//! Unified notification policy — the single choke point every notification-
//! producing feature routes through, so category preferences, quiet hours,
//! privacy redaction, deduplication, and resolution are decided in ONE place
//! instead of each feature inventing its own rules.
//!
//! # `dedup_key` has two shapes — pick the right one
//!
//! - **Standing condition** — a state that stays true until it clears (stale
//!   data, a cash-flow shortfall). Use a **stable** key (`"stale.{account}"`). A
//!   repeat while unresolved is suppressed; the producer must call [`resolve`] at
//!   the site where the condition *clears* (a different code path from where it
//!   was raised). `expires_at` is an optional backstop.
//! - **Discrete event** — something that happened once (a subscription price
//!   change, a renewal). Encode the **instance** in the key
//!   (`"sub.price.{merchant}.{effective_date}"`) and set `expires_at`. A later,
//!   different event has a different key, so it is *not* suppressed; [`resolve`]
//!   does not apply — an event never "clears".
//!
//! Getting this wrong is the classic failure: a stable key on a discrete event
//! silently swallows the second price change; a per-instance key on a standing
//! condition re-notifies forever.

use crate::error::CoreResult;
use crate::settings;
use chrono::{DateTime, Timelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeSet;
use uuid::Uuid;

/// What a notification is about. Drives the per-category user preference and how
/// events are grouped. The string form is the stored value and the preference key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NotificationCategory {
    CashflowRisk,
    StaleData,
    DebtDeadline,
    SubscriptionChange,
    Categorization,
    GoalProgress,
    MonthEndReview,
    Security,
    SyncError,
    AccountActivity,
    /// A batched summary of routine items (#69). Not a content category — it's a
    /// delivery mechanism controlled by `digest_frequency`, so it's kept out of
    /// the per-category toggle list and can't be individually disabled.
    Digest,
}

impl NotificationCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CashflowRisk => "cashflow_risk",
            Self::StaleData => "stale_data",
            Self::DebtDeadline => "debt_deadline",
            Self::SubscriptionChange => "subscription_change",
            Self::Categorization => "categorization",
            Self::GoalProgress => "goal_progress",
            Self::MonthEndReview => "month_end_review",
            Self::Security => "security",
            Self::SyncError => "sync_error",
            Self::AccountActivity => "account_activity",
            Self::Digest => "digest",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        [Self::Digest].iter().copied().chain(Self::ALL).find(|c| c.as_str() == s)
    }
    /// Every USER-FACING category, for building the preferences UI. `Digest` is
    /// deliberately excluded — it's a delivery mode, not a content toggle.
    pub const ALL: [NotificationCategory; 10] = [
        Self::CashflowRisk,
        Self::StaleData,
        Self::DebtDeadline,
        Self::SubscriptionChange,
        Self::Categorization,
        Self::GoalProgress,
        Self::MonthEndReview,
        Self::Security,
        Self::SyncError,
        Self::AccountActivity,
    ];
    /// Human-readable label for the settings UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::CashflowRisk => "Cash-flow risk",
            Self::StaleData => "Stale account data",
            Self::DebtDeadline => "Debt deadlines",
            Self::SubscriptionChange => "Subscription changes",
            Self::Categorization => "Categorization to review",
            Self::GoalProgress => "Goal progress",
            Self::MonthEndReview => "Month-end review",
            Self::Security => "Security",
            Self::SyncError => "Sync errors",
            Self::AccountActivity => "New account activity",
            Self::Digest => "Digest",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    /// Bypasses quiet hours (e.g. security).
    Critical,
    Normal,
    Low,
}

impl Urgency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Normal => "normal",
            Self::Low => "low",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "critical" => Self::Critical,
            "low" => Self::Low,
            _ => Self::Normal,
        }
    }
}

/// How much sensitive detail a notification may expose. Mirrors the app's
/// "blur amounts" privacy concept rather than inventing a parallel model — a
/// push composed server-side can't consult the client's blur, so this is the
/// server-side equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyLevel {
    /// Full detail — amounts and merchants may appear.
    Full,
    /// Redact the sensitive amount/merchant from outbound content (push, etc.).
    HideAmounts,
}

impl PrivacyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::HideAmounts => "hide_amounts",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "hide_amounts" => Self::HideAmounts,
            _ => Self::Full,
        }
    }
}

/// A notification a producer wants to raise. `body` must be safe on its own;
/// `sensitive` is the amount/merchant that privacy redaction may drop.
#[derive(Debug, Clone)]
pub struct NewNotification {
    pub category: NotificationCategory,
    pub urgency: Urgency,
    pub dedup_key: String,
    pub title: String,
    pub body: String,
    pub sensitive: Option<String>,
    pub route: Option<String>,
    /// RFC3339 self-expiry (mainly for discrete events).
    pub expires_at: Option<String>,
}

/// How often to batch routine notifications into one summary instead of pushing
/// each individually (#69).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DigestFrequency {
    /// Push each notification as it happens (the default).
    Off,
    Daily,
    Weekly,
}

impl DigestFrequency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "daily" => Self::Daily,
            "weekly" => Self::Weekly,
            _ => Self::Off,
        }
    }
    /// The digest interval in days, or `None` when digests are off.
    pub fn interval_days(self) -> Option<i64> {
        match self {
            Self::Off => None,
            Self::Daily => Some(1),
            Self::Weekly => Some(7),
        }
    }
}

/// User preferences that gate delivery.
#[derive(Debug, Clone)]
pub struct Prefs {
    pub master_enabled: bool,
    pub disabled_categories: BTreeSet<String>,
    /// Local `(start_hour, end_hour)` 0–23 during which non-critical
    /// notifications are held. `start == end` means no quiet window.
    pub quiet_hours: Option<(u8, u8)>,
    /// A one-off "quiet until this RFC3339 instant" that suppresses individual
    /// pushes of non-critical notifications, distinct from the recurring
    /// quiet-hours window (#69). `None`, or a past instant, means not snoozed.
    pub snooze_until: Option<String>,
    /// Batch routine notifications into a periodic summary instead of pushing
    /// each one (#69).
    pub digest_frequency: DigestFrequency,
    /// The client's offset from UTC in minutes (`local = UTC + offset`), captured
    /// when prefs were last saved. The server has no other way to know the user's
    /// clock — a Dockerized server runs UTC regardless — so quiet-hour boundaries,
    /// which the user sets in *local* time, are evaluated against this. DST-naive
    /// by design: re-saved on every pref change, and hour-granularity windows
    /// tolerate the twice-a-year hour of drift.
    pub utc_offset_minutes: i32,
    pub privacy: PrivacyLevel,
}

impl Prefs {
    pub fn category_enabled(&self, c: NotificationCategory) -> bool {
        self.master_enabled && !self.disabled_categories.contains(c.as_str())
    }
}

/// What happened to an `enqueue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// Recorded and ready to surface/push now.
    Delivered,
    /// Recorded but held for quiet hours or a snooze; surfaces in the in-app
    /// history only.
    Held,
    /// Recorded and surfaced in-app, but its individual push is withheld because
    /// a digest is on — the periodic digest will announce it (#69).
    Batched,
    /// The same unresolved condition/event is already present.
    SuppressedDuplicate,
    /// The category (or all notifications) is turned off; nothing recorded.
    SuppressedDisabled,
}

pub struct EnqueueOutcome {
    /// The row id, unless fully suppressed by a disabled category.
    pub id: Option<String>,
    pub disposition: Disposition,
    /// Whether the caller's push channel may send this now.
    pub push: bool,
}

/// A stored notification, as the history surface reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub category: NotificationCategory,
    pub urgency: Urgency,
    pub title: String,
    pub body: String,
    pub sensitive: Option<String>,
    pub route: Option<String>,
    pub created_at: String,
    pub delivered_at: Option<String>,
    pub read_at: Option<String>,
    pub resolved_at: Option<String>,
}

const KEY_ENABLED: &str = "notifications.enabled";
const KEY_DISABLED_CATEGORIES: &str = "notifications.disabled_categories";
const KEY_QUIET_HOURS: &str = "notifications.quiet_hours";
const KEY_UTC_OFFSET: &str = "notifications.utc_offset_minutes";
const KEY_PRIVACY: &str = "notifications.privacy";
const KEY_SNOOZE_UNTIL: &str = "notifications.snooze_until";
const KEY_DIGEST_FREQUENCY: &str = "notifications.digest_frequency";
/// Internal marker (not user-set): when the last digest was produced.
const KEY_LAST_DIGEST_AT: &str = "notifications.last_digest_at";

/// Widest real-world UTC offset span (UTC−12 … UTC+14), used to reject garbage.
const MIN_OFFSET_MINUTES: i32 = -12 * 60;
const MAX_OFFSET_MINUTES: i32 = 14 * 60;

/// Clamp an offset to the real-world range; anything outside collapses to UTC.
pub fn sanitize_offset_minutes(m: i32) -> i32 {
    if (MIN_OFFSET_MINUTES..=MAX_OFFSET_MINUTES).contains(&m) { m } else { 0 }
}

/// Load preferences from settings. `master_enabled` reuses the pre-existing
/// `notifications.enabled` toggle so an upgrading user is neither silently
/// opted in nor out; unset categories default to ON.
pub fn load_prefs(conn: &Connection) -> Prefs {
    let master_enabled = settings::get::<bool>(conn, KEY_ENABLED).ok().flatten().unwrap_or(true);
    let disabled_categories = settings::get::<Vec<String>>(conn, KEY_DISABLED_CATEGORIES)
        .ok()
        .flatten()
        .unwrap_or_default()
        .into_iter()
        .collect();
    let quiet_hours = settings::get::<[u8; 2]>(conn, KEY_QUIET_HOURS)
        .ok()
        .flatten()
        .and_then(|[a, b]| (a < 24 && b < 24 && a != b).then_some((a, b)));
    let utc_offset_minutes = settings::get::<i32>(conn, KEY_UTC_OFFSET)
        .ok()
        .flatten()
        .map(sanitize_offset_minutes)
        .unwrap_or(0);
    let privacy = settings::get::<String>(conn, KEY_PRIVACY)
        .ok()
        .flatten()
        .map(|s| PrivacyLevel::from_str(&s))
        .unwrap_or(PrivacyLevel::Full);
    let snooze_until = settings::get::<String>(conn, KEY_SNOOZE_UNTIL).ok().flatten().filter(|s| !s.is_empty());
    let digest_frequency = settings::get::<String>(conn, KEY_DIGEST_FREQUENCY)
        .ok()
        .flatten()
        .map(|s| DigestFrequency::from_str(&s))
        .unwrap_or(DigestFrequency::Off);
    Prefs {
        master_enabled,
        disabled_categories,
        quiet_hours,
        utc_offset_minutes,
        privacy,
        snooze_until,
        digest_frequency,
    }
}

/// Persist preferences.
pub fn save_prefs(conn: &Connection, prefs: &Prefs) -> CoreResult<()> {
    settings::set(conn, KEY_ENABLED, &prefs.master_enabled)?;
    let disabled: Vec<&str> = prefs.disabled_categories.iter().map(String::as_str).collect();
    settings::set(conn, KEY_DISABLED_CATEGORIES, &disabled)?;
    match prefs.quiet_hours {
        Some((a, b)) => settings::set(conn, KEY_QUIET_HOURS, &[a, b])?,
        None => settings::set(conn, KEY_QUIET_HOURS, &Option::<[u8; 2]>::None)?,
    }
    settings::set(conn, KEY_UTC_OFFSET, &sanitize_offset_minutes(prefs.utc_offset_minutes))?;
    settings::set(conn, KEY_PRIVACY, &prefs.privacy.as_str())?;
    settings::set(conn, KEY_SNOOZE_UNTIL, &prefs.snooze_until.clone().unwrap_or_default())?;
    settings::set(conn, KEY_DIGEST_FREQUENCY, &prefs.digest_frequency.as_str())?;
    Ok(())
}

/// Whether `now` falls in the user's quiet window. The window is expressed in
/// the user's LOCAL time, so `now` is shifted by their captured UTC offset before
/// the hour is read. Handles windows that wrap past midnight (e.g. 22→7).
fn in_quiet_hours(quiet: Option<(u8, u8)>, offset_minutes: i32, now: DateTime<Utc>) -> bool {
    let Some((start, end)) = quiet else { return false };
    let local = now + chrono::Duration::minutes(offset_minutes as i64);
    let h = local.hour() as u8;
    if start < end {
        h >= start && h < end
    } else {
        // Wraps midnight.
        h >= start || h < end
    }
}

/// Whether a one-off snooze is currently active (`now` is before the snooze
/// instant). A missing or unparseable value, or a past instant, is not snoozed.
fn in_snooze(snooze_until: Option<&str>, now: DateTime<Utc>) -> bool {
    snooze_until
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map_or(false, |until| now < until.with_timezone(&Utc))
}

/// Compose the outbound text for a channel that can't blur amounts itself
/// (push, OS notifications), honoring the privacy level.
pub fn redact(body: &str, sensitive: Option<&str>, privacy: PrivacyLevel) -> String {
    match (privacy, sensitive) {
        (PrivacyLevel::Full, Some(s)) if !s.is_empty() => format!("{body} {s}"),
        _ => body.to_string(),
    }
}

/// The gate. Applies category preferences → dedup (against unresolved rows) →
/// quiet hours, records the notification, and reports whether the caller may
/// push it now. A disabled category records nothing.
pub fn enqueue(
    conn: &mut Connection,
    new: NewNotification,
    prefs: &Prefs,
    now: DateTime<Utc>,
) -> CoreResult<EnqueueOutcome> {
    if !prefs.category_enabled(new.category) {
        return Ok(EnqueueOutcome { id: None, disposition: Disposition::SuppressedDisabled, push: false });
    }

    // Dedup only over UNRESOLVED rows: a standing condition stays deduped until
    // resolved; a discrete event's unique key never collides in the first place.
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM notifications WHERE dedup_key = ?1 AND resolved_at IS NULL ORDER BY created_at DESC LIMIT 1",
            params![new.dedup_key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return Ok(EnqueueOutcome { id: Some(id), disposition: Disposition::SuppressedDuplicate, push: false });
    }

    let non_critical = !matches!(new.urgency, Urgency::Critical);
    // Quiet hours OR a one-off snooze HOLD a non-critical item: recorded, still
    // visible in-app, but not yet delivered/pushed.
    let held = non_critical
        && (in_quiet_hours(prefs.quiet_hours, prefs.utc_offset_minutes, now)
            || in_snooze(prefs.snooze_until.as_deref(), now));
    // A digest withholds the INDIVIDUAL push of a non-critical item (it's still
    // delivered in-app) — the periodic digest announces it. The digest
    // notification itself must never suppress its own push.
    let batched = !held
        && non_critical
        && prefs.digest_frequency != DigestFrequency::Off
        && new.category != NotificationCategory::Digest;
    let id = Uuid::new_v4().to_string();
    let now_str = now.to_rfc3339();
    let delivered_at = if held { None } else { Some(now_str.clone()) };
    conn.execute(
        "INSERT INTO notifications(id, category, urgency, dedup_key, title, body, sensitive, route, created_at, delivered_at, expires_at) \
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            id,
            new.category.as_str(),
            new.urgency.as_str(),
            new.dedup_key,
            new.title,
            new.body,
            new.sensitive,
            new.route,
            now_str,
            delivered_at,
            new.expires_at,
        ],
    )?;
    Ok(EnqueueOutcome {
        id: Some(id),
        disposition: if held {
            Disposition::Held
        } else if batched {
            Disposition::Batched
        } else {
            Disposition::Delivered
        },
        push: !held && !batched,
    })
}

/// Mark every unresolved notification with this dedup key resolved — call at the
/// site where a standing condition clears. Returns how many were resolved.
pub fn resolve(conn: &mut Connection, dedup_key: &str) -> CoreResult<usize> {
    let n = conn.execute(
        "UPDATE notifications SET resolved_at = ?2 WHERE dedup_key = ?1 AND resolved_at IS NULL",
        params![dedup_key, Utc::now().to_rfc3339()],
    )?;
    Ok(n)
}

/// Auto-resolve anything whose `expires_at` has passed — the backstop for
/// discrete events and conditions with no explicit clearing signal.
pub fn expire_due(conn: &mut Connection, now: DateTime<Utc>) -> CoreResult<usize> {
    let n = conn.execute(
        "UPDATE notifications SET resolved_at = ?1 \
         WHERE resolved_at IS NULL AND expires_at IS NOT NULL AND expires_at <= ?1",
        params![now.to_rfc3339()],
    )?;
    Ok(n)
}

/// Standing-condition producer: raise a `StaleData` notification for each
/// connected account whose most recent successful sync is older than
/// `threshold_days` (or which has never synced), and RESOLVE it for any account
/// that is now fresh. Idempotent — safe to run after every sync cycle. This is
/// the canonical two-site standing-condition lifecycle: the condition is raised
/// and cleared from one place, driven by the account's own last-sync age.
/// In-app only (no push) — stale data isn't lock-screen-urgent.
pub fn refresh_stale_accounts(conn: &mut Connection, threshold_days: i64, now: DateTime<Utc>) -> CoreResult<()> {
    let prefs = load_prefs(conn);
    let threshold = threshold_days.max(1);
    let cutoff = now - chrono::Duration::days(threshold);
    let accounts: Vec<(String, String, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT id, name, last_synced_at FROM accounts \
             WHERE archived_at IS NULL AND simplefin_account_id IS NOT NULL",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for (id, name, last_synced) in accounts {
        let key = format!("stale.{id}");
        let is_stale = match last_synced.as_deref() {
            None => true,
            Some(ts) => DateTime::parse_from_rfc3339(ts)
                .map(|d| d.with_timezone(&Utc) < cutoff)
                .unwrap_or(true),
        };
        if is_stale {
            enqueue(
                conn,
                NewNotification {
                    category: NotificationCategory::StaleData,
                    urgency: Urgency::Normal,
                    dedup_key: key,
                    title: "Account data may be stale".into(),
                    body: format!("{name} hasn't synced in over {threshold} day(s) — its balances and totals may be out of date."),
                    sensitive: None,
                    route: Some("/accounts".into()),
                    expires_at: None,
                },
                &prefs,
                now,
            )?;
        } else {
            resolve(conn, &key)?;
        }
    }
    Ok(())
}

/// Digest producer (#69): when digests are enabled and one is due, push a single
/// summary of the routine notifications accumulated since the last digest
/// (unread, non-critical, unresolved, un-expired) — instead of having buzzed for
/// each. Advances the marker every run, so a due-but-empty window just resets
/// the clock. Returns 1 if a digest was pushed, else 0. Drive it from the same
/// periodic sweep as the other standing producers.
pub fn build_digest(conn: &mut Connection, now: DateTime<Utc>) -> CoreResult<usize> {
    let prefs = load_prefs(conn);
    let Some(interval_days) = prefs.digest_frequency.interval_days() else {
        return Ok(0);
    };
    let last = settings::get::<String>(conn, KEY_LAST_DIGEST_AT)
        .ok()
        .flatten()
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc));
    if let Some(last) = last {
        if (now - last).num_days() < interval_days {
            return Ok(0); // not due yet
        }
    }
    let now_str = now.to_rfc3339();
    // Everything routine the user hasn't seen since the last digest (an empty
    // `since` on the very first digest means "everything so far").
    let since = last.map(|d| d.to_rfc3339()).unwrap_or_default();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notifications \
         WHERE resolved_at IS NULL AND read_at IS NULL AND urgency != 'critical' \
           AND category != 'digest' AND created_at > ?1 \
           AND (expires_at IS NULL OR expires_at > ?2)",
        params![since, now_str],
        |r| r.get(0),
    )?;
    settings::set(conn, KEY_LAST_DIGEST_AT, &now_str)?;
    if count == 0 {
        return Ok(0);
    }
    let outcome = enqueue(
        conn,
        NewNotification {
            category: NotificationCategory::Digest,
            urgency: Urgency::Normal,
            dedup_key: format!("digest.{now_str}"),
            title: "Your notifications digest".into(),
            body: format!(
                "{count} update{} you haven't seen since your last digest.",
                if count == 1 { "" } else { "s" }
            ),
            sensitive: None,
            route: Some("/".into()),
            // Lapses after one interval so a stale digest clears itself.
            expires_at: Some((now + chrono::Duration::days(interval_days)).to_rfc3339()),
        },
        &prefs,
        now,
    )?;
    Ok(if outcome.push { 1 } else { 0 })
}

fn row_to_notification(r: &rusqlite::Row) -> rusqlite::Result<Notification> {
    Ok(Notification {
        id: r.get(0)?,
        category: NotificationCategory::from_str(&r.get::<_, String>(1)?).unwrap_or(NotificationCategory::AccountActivity),
        urgency: Urgency::from_str(&r.get::<_, String>(2)?),
        title: r.get(3)?,
        body: r.get(4)?,
        sensitive: r.get(5)?,
        route: r.get(6)?,
        created_at: r.get(7)?,
        delivered_at: r.get(8)?,
        read_at: r.get(9)?,
        resolved_at: r.get(10)?,
    })
}

const SELECT_COLS: &str =
    "id, category, urgency, title, body, sensitive, route, created_at, delivered_at, read_at, resolved_at";

/// Notification history, newest first. `include_resolved=false` shows only
/// still-active items (the default center view) — which excludes anything past
/// its `expires_at` even if [`expire_due`] hasn't swept it yet, so the active
/// view is correct without depending on the sweep's timing.
pub fn list(conn: &mut Connection, include_resolved: bool, limit: i64) -> CoreResult<Vec<Notification>> {
    let now = Utc::now().to_rfc3339();
    let mut stmt;
    let rows = if include_resolved {
        let sql = format!("SELECT {SELECT_COLS} FROM notifications ORDER BY created_at DESC LIMIT ?1");
        stmt = conn.prepare(&sql)?;
        stmt.query_map(params![limit.clamp(1, 500)], row_to_notification)?
    } else {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM notifications \
             WHERE resolved_at IS NULL AND (expires_at IS NULL OR expires_at > ?2) \
             ORDER BY created_at DESC LIMIT ?1"
        );
        stmt = conn.prepare(&sql)?;
        stmt.query_map(params![limit.clamp(1, 500), now], row_to_notification)?
    };
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn mark_read(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE notifications SET read_at = ?2 WHERE id = ?1 AND read_at IS NULL",
        params![id, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn mark_all_read(conn: &mut Connection) -> CoreResult<usize> {
    let n = conn.execute(
        "UPDATE notifications SET read_at = ?1 WHERE read_at IS NULL AND resolved_at IS NULL",
        params![Utc::now().to_rfc3339()],
    )?;
    Ok(n)
}

/// Unread, unresolved count — what the app-icon badge reflects. Excludes
/// expired-but-unswept rows for the same reason [`list`] does, so the badge and
/// the active view never disagree.
pub fn unread_count(conn: &mut Connection) -> CoreResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM notifications \
         WHERE read_at IS NULL AND resolved_at IS NULL AND (expires_at IS NULL OR expires_at > ?1)",
        params![Utc::now().to_rfc3339()],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("notify.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn prefs() -> Prefs {
        Prefs {
            master_enabled: true,
            disabled_categories: BTreeSet::new(),
            quiet_hours: None,
            utc_offset_minutes: 0,
            privacy: PrivacyLevel::Full,
            snooze_until: None,
            digest_frequency: DigestFrequency::Off,
        }
    }

    fn note(category: NotificationCategory, dedup_key: &str, urgency: Urgency) -> NewNotification {
        NewNotification {
            category,
            urgency,
            dedup_key: dedup_key.into(),
            title: "T".into(),
            body: "B".into(),
            sensitive: Some("$1,234".into()),
            route: None,
            expires_at: None,
        }
    }

    fn now() -> DateTime<Utc> {
        "2026-08-01T14:00:00Z".parse().unwrap() // 2pm — outside a 22-7 quiet window
    }

    // ── #69: snooze + digests ───────────────────────────────────────────────

    #[test]
    fn snooze_holds_non_critical_push_until_it_lapses() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let mut p = prefs();
        p.snooze_until = Some("2026-08-01T15:00:00Z".into()); // snoozed until 3pm; now is 2pm
        let out = enqueue(&mut c, note(NotificationCategory::CashflowRisk, "cf.1", Urgency::Normal), &p, now()).unwrap();
        assert_eq!(out.disposition, Disposition::Held);
        assert!(!out.push);
        assert_eq!(unread_count(&mut c).unwrap(), 1, "held, but still visible in-app");
        // Critical bypasses the snooze.
        let crit = enqueue(&mut c, note(NotificationCategory::Security, "sec.1", Urgency::Critical), &p, now()).unwrap();
        assert!(crit.push);
        // A snooze already in the past doesn't hold.
        p.snooze_until = Some("2026-08-01T13:00:00Z".into());
        let out2 = enqueue(&mut c, note(NotificationCategory::CashflowRisk, "cf.2", Urgency::Normal), &p, now()).unwrap();
        assert!(out2.push);
    }

    #[test]
    fn digest_batches_non_critical_pushes_but_keeps_them_visible() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let mut p = prefs();
        p.digest_frequency = DigestFrequency::Daily;
        let out = enqueue(&mut c, note(NotificationCategory::GoalProgress, "g.1", Urgency::Low), &p, now()).unwrap();
        assert_eq!(out.disposition, Disposition::Batched);
        assert!(!out.push);
        assert_eq!(unread_count(&mut c).unwrap(), 1, "batched, but still visible/unread in-app");
        // Critical still pushes individually even with a digest on.
        let crit = enqueue(&mut c, note(NotificationCategory::Security, "sec.1", Urgency::Critical), &p, now()).unwrap();
        assert!(crit.push);
    }

    #[test]
    fn build_digest_summarizes_batched_items_when_due_and_only_then() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let mut p = prefs();
        p.digest_frequency = DigestFrequency::Daily;
        save_prefs(&c, &p).unwrap();
        enqueue(&mut c, note(NotificationCategory::GoalProgress, "g.1", Urgency::Low), &p, now()).unwrap();
        enqueue(&mut c, note(NotificationCategory::Categorization, "cat.1", Urgency::Normal), &p, now()).unwrap();
        // First digest is due (never run) and summarizes both, with a push.
        assert_eq!(build_digest(&mut c, now()).unwrap(), 1);
        let all = list(&mut c, true, 50).unwrap();
        let digest = all.iter().find(|x| x.category == NotificationCategory::Digest).expect("digest created");
        assert!(digest.body.contains('2'));
        // Running again immediately is not due (daily) → no second digest.
        assert_eq!(build_digest(&mut c, now()).unwrap(), 0);
    }

    #[test]
    fn digest_off_leaves_individual_pushes_untouched() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let p = prefs(); // digest Off by default
        let out = enqueue(&mut c, note(NotificationCategory::GoalProgress, "g.1", Urgency::Low), &p, now()).unwrap();
        assert_eq!(out.disposition, Disposition::Delivered);
        assert!(out.push);
        assert_eq!(build_digest(&mut c, now()).unwrap(), 0, "no digest when off");
    }

    #[test]
    fn delivered_when_enabled_and_not_quiet() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let out = enqueue(&mut c, note(NotificationCategory::CashflowRisk, "cf.1", Urgency::Normal), &prefs(), now()).unwrap();
        assert_eq!(out.disposition, Disposition::Delivered);
        assert!(out.push && out.id.is_some());
        assert_eq!(unread_count(&mut c).unwrap(), 1);
    }

    #[test]
    fn disabled_category_records_nothing() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let mut p = prefs();
        p.disabled_categories.insert("cashflow_risk".into());
        let out = enqueue(&mut c, note(NotificationCategory::CashflowRisk, "cf.1", Urgency::Normal), &p, now()).unwrap();
        assert_eq!(out.disposition, Disposition::SuppressedDisabled);
        assert!(out.id.is_none());
        assert_eq!(unread_count(&mut c).unwrap(), 0);
        // Master off suppresses too.
        p.disabled_categories.clear();
        p.master_enabled = false;
        assert_eq!(
            enqueue(&mut c, note(NotificationCategory::StaleData, "s.1", Urgency::Normal), &p, now()).unwrap().disposition,
            Disposition::SuppressedDisabled
        );
    }

    /// STANDING condition: a stable key is deduped while unresolved, then a fresh
    /// enqueue after `resolve` is delivered again (the condition recurred).
    #[test]
    fn standing_condition_dedups_until_resolved() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let key = "stale.acct-1";
        assert_eq!(enqueue(&mut c, note(NotificationCategory::StaleData, key, Urgency::Normal), &prefs(), now()).unwrap().disposition, Disposition::Delivered);
        // Same condition again → suppressed, no second row.
        assert_eq!(enqueue(&mut c, note(NotificationCategory::StaleData, key, Urgency::Normal), &prefs(), now()).unwrap().disposition, Disposition::SuppressedDuplicate);
        assert_eq!(list(&mut c, false, 50).unwrap().len(), 1);

        // Condition clears, then recurs → delivered again.
        assert_eq!(resolve(&mut c, key).unwrap(), 1);
        assert_eq!(list(&mut c, false, 50).unwrap().len(), 0); // active list empty
        assert_eq!(enqueue(&mut c, note(NotificationCategory::StaleData, key, Urgency::Normal), &prefs(), now()).unwrap().disposition, Disposition::Delivered);
    }

    /// DISCRETE events: two different price changes must BOTH notify — a stable
    /// key would have swallowed the second.
    #[test]
    fn discrete_events_with_distinct_keys_both_deliver() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let first = enqueue(&mut c, note(NotificationCategory::SubscriptionChange, "sub.price.spotify.2026-07-01", Urgency::Normal), &prefs(), now()).unwrap();
        let second = enqueue(&mut c, note(NotificationCategory::SubscriptionChange, "sub.price.spotify.2026-09-01", Urgency::Normal), &prefs(), now()).unwrap();
        assert_eq!(first.disposition, Disposition::Delivered);
        assert_eq!(second.disposition, Disposition::Delivered);
        assert_eq!(list(&mut c, false, 50).unwrap().len(), 2);
    }

    #[test]
    fn quiet_hours_holds_normal_but_not_critical() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let mut p = prefs();
        p.quiet_hours = Some((22, 7)); // wraps midnight
        let night: DateTime<Utc> = "2026-08-01T23:30:00Z".parse().unwrap();

        let normal = enqueue(&mut c, note(NotificationCategory::AccountActivity, "a.1", Urgency::Normal), &p, night).unwrap();
        assert_eq!(normal.disposition, Disposition::Held);
        assert!(!normal.push);
        // Held items still show in history (so they're not lost).
        assert_eq!(list(&mut c, false, 50).unwrap().len(), 1);

        let critical = enqueue(&mut c, note(NotificationCategory::Security, "sec.1", Urgency::Critical), &p, night).unwrap();
        assert_eq!(critical.disposition, Disposition::Delivered);
        assert!(critical.push);
    }

    /// Quiet hours are the USER'S LOCAL time, not the server's UTC. A user in
    /// UTC−8 with a 22:00–07:00 window must be held at their 3am and delivered at
    /// their 2pm — the exact hours a UTC-only check gets backwards. Without the
    /// offset shift this test fails, which is the whole point.
    #[test]
    fn quiet_hours_respect_local_offset_not_utc() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let mut p = prefs();
        p.quiet_hours = Some((22, 7));
        p.utc_offset_minutes = -8 * 60; // UTC−8 (e.g. PST)

        // 06:00 UTC → 22:00 local (previous day) → inside the window → held.
        let at_local_10pm: DateTime<Utc> = "2026-08-02T06:00:00Z".parse().unwrap();
        let held = enqueue(&mut c, note(NotificationCategory::AccountActivity, "night", Urgency::Normal), &p, at_local_10pm).unwrap();
        assert_eq!(held.disposition, Disposition::Held, "10pm local must be quiet");

        // 22:00 UTC → 14:00 local → OUTSIDE the window → delivered. A UTC-only
        // check would wrongly hold this (UTC hour 22 is inside 22–7).
        let at_local_2pm: DateTime<Utc> = "2026-08-02T22:00:00Z".parse().unwrap();
        let delivered = enqueue(&mut c, note(NotificationCategory::AccountActivity, "afternoon", Urgency::Normal), &p, at_local_2pm).unwrap();
        assert_eq!(delivered.disposition, Disposition::Delivered, "2pm local must NOT be quiet");

        // Half-hour offset (UTC+5:30) resolves to the right hour too.
        p.utc_offset_minutes = 5 * 60 + 30;
        // 17:00 UTC → 22:30 local → inside → held.
        let india_night: DateTime<Utc> = "2026-08-02T17:00:00Z".parse().unwrap();
        let held_in = enqueue(&mut c, note(NotificationCategory::AccountActivity, "india", Urgency::Normal), &p, india_night).unwrap();
        assert_eq!(held_in.disposition, Disposition::Held, "10:30pm IST must be quiet");
    }

    #[test]
    fn offset_sanitizer_rejects_garbage() {
        assert_eq!(sanitize_offset_minutes(-480), -480);
        assert_eq!(sanitize_offset_minutes(330), 330);
        assert_eq!(sanitize_offset_minutes(14 * 60), 14 * 60);
        // Out of the real-world span collapses to UTC rather than shifting wildly.
        assert_eq!(sanitize_offset_minutes(99_999), 0);
        assert_eq!(sanitize_offset_minutes(-99_999), 0);
    }

    #[test]
    fn redaction_honors_privacy() {
        assert_eq!(redact("Balance low", Some("$50"), PrivacyLevel::Full), "Balance low $50");
        assert_eq!(redact("Balance low", Some("$50"), PrivacyLevel::HideAmounts), "Balance low");
        assert_eq!(redact("Balance low", None, PrivacyLevel::Full), "Balance low");
    }

    #[test]
    fn expiry_backstop_resolves_past_due() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let mut n = note(NotificationCategory::SubscriptionChange, "sub.renewal.x.2026-08", Urgency::Low);
        n.expires_at = Some("2026-08-05T00:00:00Z".to_string());
        enqueue(&mut c, n, &prefs(), now()).unwrap();
        // Before expiry: still active.
        assert_eq!(expire_due(&mut c, "2026-08-04T00:00:00Z".parse().unwrap()).unwrap(), 0);
        // After expiry: auto-resolved.
        assert_eq!(expire_due(&mut c, "2026-08-06T00:00:00Z".parse().unwrap()).unwrap(), 1);
        assert_eq!(list(&mut c, false, 50).unwrap().len(), 0);
    }

    /// Per-category resolve test (StaleData): the condition is raised for a
    /// stale connected account and RESOLVED once it syncs fresh — proving both
    /// ends of the standing-condition lifecycle exist.
    #[test]
    fn stale_accounts_notify_then_resolve_on_fresh_sync() {
        use chrono::Duration;
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let ins = |c: &Connection, id: &str, name: &str, sfin: Option<&str>, synced: Option<String>| {
            c.execute(
                "INSERT INTO accounts(id,owner,bank,type,name,color,created_at,simplefin_account_id,last_synced_at) \
                 VALUES(?1,'me','B','Checking',?2,'#fff',datetime('now'),?3,?4)",
                params![id, name, sfin, synced],
            )
            .unwrap();
        };
        ins(&c, "a-stale", "Old Checking", Some("sf1"), Some((Utc::now() - Duration::days(10)).to_rfc3339()));
        ins(&c, "a-fresh", "New Checking", Some("sf2"), Some(Utc::now().to_rfc3339()));
        // A non-connected (manual) account must be ignored entirely.
        ins(&c, "a-manual", "Manual", None, None);

        refresh_stale_accounts(&mut c, 3, Utc::now()).unwrap();
        let active = list(&mut c, false, 50).unwrap();
        assert_eq!(active.len(), 1, "only the stale CONNECTED account notifies");
        assert_eq!(active[0].category, NotificationCategory::StaleData);
        assert!(active[0].body.contains("Old Checking"));

        // Idempotent — a second cycle doesn't duplicate.
        refresh_stale_accounts(&mut c, 3, Utc::now()).unwrap();
        assert_eq!(list(&mut c, false, 50).unwrap().len(), 1);

        // The account syncs → the same call RESOLVES it (the clearing site).
        c.execute("UPDATE accounts SET last_synced_at = ?1 WHERE id = 'a-stale'", params![Utc::now().to_rfc3339()]).unwrap();
        refresh_stale_accounts(&mut c, 3, Utc::now()).unwrap();
        assert_eq!(list(&mut c, false, 50).unwrap().len(), 0, "resolved once the account is fresh again");
    }

    #[test]
    fn read_state_and_prefs_round_trip() {
        let (_d, db) = fresh();
        let mut c = db.get().unwrap();
        let out = enqueue(&mut c, note(NotificationCategory::GoalProgress, "g.1", Urgency::Low), &prefs(), now()).unwrap();
        assert_eq!(unread_count(&mut c).unwrap(), 1);
        mark_read(&mut c, out.id.as_deref().unwrap()).unwrap();
        assert_eq!(unread_count(&mut c).unwrap(), 0);

        let mut p = prefs();
        p.disabled_categories.insert("sync_error".into());
        p.quiet_hours = Some((23, 6));
        p.utc_offset_minutes = -5 * 60; // UTC−5
        p.privacy = PrivacyLevel::HideAmounts;
        save_prefs(&c, &p).unwrap();
        let loaded = load_prefs(&c);
        assert!(!loaded.category_enabled(NotificationCategory::SyncError));
        assert_eq!(loaded.quiet_hours, Some((23, 6)));
        assert_eq!(loaded.utc_offset_minutes, -5 * 60);
        assert_eq!(loaded.privacy, PrivacyLevel::HideAmounts);
    }
}
