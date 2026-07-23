//! Frontend bridge to the unified notification policy (`finsight-core::notify`):
//! read/write preferences, list the notification history, and drive the badge.
//! Producers (sync scheduler, #58/#59, …) enqueue through the core policy, never
//! through here.

use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::notify::{
    self, DigestFrequency, Notification, NotificationCategory, Prefs, PrivacyLevel,
};
use finsight_core::repos::run;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NotificationCategoryPref {
    /// Stable key (e.g. "cashflow_risk").
    pub key: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QuietHours {
    /// Local hour 0–23 the quiet window starts.
    pub start: u8,
    /// Local hour 0–23 it ends. Equal to `start` means "no window".
    pub end: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPrefsDto {
    pub master_enabled: bool,
    /// Every category with its current enabled state, in a stable order for the UI.
    pub categories: Vec<NotificationCategoryPref>,
    pub quiet_hours: Option<QuietHours>,
    /// The client's UTC offset in minutes (`local = UTC + offset`). The client
    /// stamps this on every save so the server can evaluate the local-time quiet
    /// window; the user never edits it directly.
    pub utc_offset_minutes: i32,
    pub privacy: PrivacyLevel,
    /// A one-off "snooze until" RFC3339 instant, or null. While active it holds
    /// individual pushes of non-critical notifications (#69). The client sets it
    /// to `now + duration`; a past value is treated as not snoozed.
    pub snooze_until: Option<String>,
    /// Batch routine notifications into a periodic summary instead of pushing
    /// each one (#69): "off" | "daily" | "weekly".
    pub digest_frequency: DigestFrequency,
}

fn prefs_to_dto(p: &Prefs) -> NotificationPrefsDto {
    NotificationPrefsDto {
        master_enabled: p.master_enabled,
        categories: NotificationCategory::ALL
            .iter()
            .map(|c| NotificationCategoryPref {
                key: c.as_str().to_string(),
                label: c.label().to_string(),
                enabled: !p.disabled_categories.contains(c.as_str()),
            })
            .collect(),
        quiet_hours: p.quiet_hours.map(|(start, end)| QuietHours { start, end }),
        utc_offset_minutes: p.utc_offset_minutes,
        privacy: p.privacy,
        snooze_until: p.snooze_until.clone(),
        digest_frequency: p.digest_frequency,
    }
}

fn dto_to_prefs(dto: NotificationPrefsDto) -> Prefs {
    Prefs {
        master_enabled: dto.master_enabled,
        // Only keys that are real categories AND turned off. Clamp defensively.
        disabled_categories: dto
            .categories
            .iter()
            .filter(|c| !c.enabled && NotificationCategory::from_str(&c.key).is_some())
            .map(|c| c.key.clone())
            .collect(),
        quiet_hours: dto
            .quiet_hours
            .filter(|q| q.start < 24 && q.end < 24 && q.start != q.end)
            .map(|q| (q.start, q.end)),
        utc_offset_minutes: notify::sanitize_offset_minutes(dto.utc_offset_minutes),
        privacy: dto.privacy,
        // A blank string clears the snooze; anything else is stored verbatim and
        // validated at read time (a bad/past value simply isn't snoozed).
        snooze_until: dto.snooze_until.filter(|s| !s.is_empty()),
        digest_frequency: dto.digest_frequency,
    }
}

pub async fn get_notification_prefs(state: &ApiState) -> AppResult<NotificationPrefsDto> {
    let db = (*state.db).clone();
    run(&db, |conn| Ok(prefs_to_dto(&notify::load_prefs(conn))))
        .await
        .map_err(AppError::from)
}

pub async fn set_notification_prefs(state: &ApiState, prefs: NotificationPrefsDto) -> AppResult<()> {
    let db = (*state.db).clone();
    let p = dto_to_prefs(prefs);
    run(&db, move |conn| notify::save_prefs(conn, &p))
        .await
        .map_err(AppError::from)
}

/// The notification history. `includeResolved=false` (default view) shows only
/// still-active items; held (quiet-hours) items appear here too so they're never
/// lost, just not pushed.
pub async fn list_notifications(state: &ApiState, include_resolved: Option<bool>) -> AppResult<Vec<Notification>> {
    let db = (*state.db).clone();
    let include = include_resolved.unwrap_or(false);
    run(&db, move |conn| notify::list(conn, include, 200))
        .await
        .map_err(AppError::from)
}

pub async fn mark_notification_read(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| notify::mark_read(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn mark_all_notifications_read(state: &ApiState) -> AppResult<u32> {
    let db = (*state.db).clone();
    run(&db, |conn| notify::mark_all_read(conn).map(|n| n as u32))
        .await
        .map_err(AppError::from)
}

/// Unread, unresolved count — what the installed-app icon badge reflects.
pub async fn notification_unread_count(state: &ApiState) -> AppResult<i64> {
    let db = (*state.db).clone();
    run(&db, notify::unread_count).await.map_err(AppError::from)
}
