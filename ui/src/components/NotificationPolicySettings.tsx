import { useNotificationPrefs, useSetNotificationPrefs } from "../api/hooks/notifications";
import { Toggle } from "./Toggle";
import type { NotificationPrefsDto, PrivacyLevel, DigestFrequency } from "../api/client";

const HOURS = Array.from({ length: 24 }, (_, h) => h);
const fmtHour = (h: number) => `${String(h).padStart(2, "0")}:00`;

// A sensible default window to seed the pickers with the first time quiet hours
// are switched on. The server treats start === end as "no window", so we never
// persist this unless the user opts in.
const DEFAULT_QUIET = { start: 22, end: 7 };

/**
 * The unified notification-policy panel: one master switch, a per-category
 * opt-out list, quiet hours, and a privacy choice — all driven by the single
 * `notification_prefs` contract so the choke point in `finsight-core::notify`
 * stays the sole authority on what actually gets delivered.
 *
 * Deliberately data-driven: the category rows come from the server's list, so a
 * new `NotificationCategory` shows up here automatically without a UI change.
 */
export default function NotificationPolicySettings() {
  const { data: prefs } = useNotificationPrefs();
  const setPrefs = useSetNotificationPrefs();

  // Backend unavailable (browser without a server) or first load — show the
  // master row as a placeholder rather than an empty gap.
  if (!prefs) {
    return (
      <div className="s-row">
        <div>
          <div className="label">Notifications</div>
          <div className="desc">Budget alerts, recurring reminders, and daily prompts.</div>
        </div>
        <div className="muted">—</div>
        <div />
      </div>
    );
  }

  const update = (next: NotificationPrefsDto) => setPrefs.mutate(next);
  const setCategory = (key: string, enabled: boolean) =>
    update({ ...prefs, categories: prefs.categories.map((c) => (c.key === key ? { ...c, enabled } : c)) });

  const quiet = prefs.quietHours ?? DEFAULT_QUIET;
  const quietOn = prefs.quietHours != null;

  const snoozedUntil = prefs.snoozeUntil ? new Date(prefs.snoozeUntil) : null;
  const isSnoozed = snoozedUntil != null && snoozedUntil.getTime() > Date.now();
  const snoozeFor = (ms: number) => update({ ...prefs, snoozeUntil: new Date(Date.now() + ms).toISOString() });

  const subDim = !prefs.masterEnabled;

  return (
    <>
      <div className="s-row">
        <div>
          <div className="label">Notifications enabled</div>
          <div className="desc">The master switch. Turning it off silences every category below.</div>
        </div>
        <div className="muted">{prefs.masterEnabled ? "Currently on" : "Currently off"}</div>
        <Toggle
          checked={prefs.masterEnabled}
          onChange={(v) => update({ ...prefs, masterEnabled: v })}
          ariaLabel="Notifications enabled"
        />
      </div>

      <div
        style={{ opacity: subDim ? 0.45 : 1, pointerEvents: subDim ? "none" : "auto" }}
        aria-hidden={subDim || undefined}
      >
        <div className="eyebrow" style={{ marginTop: 20, marginBottom: 4 }}>What you're notified about</div>
        {prefs.categories.map((c) => (
          <div className="s-row" key={c.key}>
            <div>
              <div className="label">{c.label}</div>
            </div>
            <div />
            <Toggle checked={c.enabled} onChange={(v) => setCategory(c.key, v)} ariaLabel={c.label} />
          </div>
        ))}

        <div className="eyebrow" style={{ marginTop: 20, marginBottom: 4 }}>Quiet hours</div>
        <div className="s-row">
          <div>
            <div className="label">Hold non-urgent alerts overnight</div>
            <div className="desc">Held notifications wait in your inbox and don't push. Urgent alerts still come through.</div>
          </div>
          <div />
          <Toggle
            checked={quietOn}
            onChange={(on) => update({ ...prefs, quietHours: on ? quiet : null })}
            ariaLabel="Quiet hours"
          />
        </div>
        {quietOn && (
          <div className="s-row">
            <div>
              <div className="label">Window</div>
              <div className="desc">Your local time.</div>
            </div>
            <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
              <label style={{ display: "inline-flex", gap: 6, alignItems: "center" }} className="muted">
                From
                <select
                  className="control"
                  aria-label="Quiet hours start"
                  value={quiet.start}
                  onChange={(e) => update({ ...prefs, quietHours: { start: Number(e.target.value), end: quiet.end } })}
                >
                  {HOURS.map((h) => <option key={h} value={h}>{fmtHour(h)}</option>)}
                </select>
              </label>
              <label style={{ display: "inline-flex", gap: 6, alignItems: "center" }} className="muted">
                to
                <select
                  className="control"
                  aria-label="Quiet hours end"
                  value={quiet.end}
                  onChange={(e) => update({ ...prefs, quietHours: { start: quiet.start, end: Number(e.target.value) } })}
                >
                  {HOURS.map((h) => <option key={h} value={h}>{fmtHour(h)}</option>)}
                </select>
              </label>
            </div>
            <div />
          </div>
        )}

        <div className="eyebrow" style={{ marginTop: 20, marginBottom: 4 }}>Snooze</div>
        <div className="s-row">
          <div>
            <div className="label">Pause notifications temporarily</div>
            <div className="desc">
              {isSnoozed
                ? `Snoozed until ${snoozedUntil!.toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}. Urgent alerts still come through.`
                : "Mute non-urgent pushes for a while. They still collect in your inbox."}
            </div>
          </div>
          <div className="toolbar">
            {isSnoozed ? (
              <button type="button" onClick={() => update({ ...prefs, snoozeUntil: null })}>Resume now</button>
            ) : (
              <>
                <button type="button" onClick={() => snoozeFor(3_600_000)}>1 hour</button>
                <button type="button" onClick={() => snoozeFor(8 * 3_600_000)}>8 hours</button>
                <button type="button" onClick={() => snoozeFor(24 * 3_600_000)}>1 day</button>
              </>
            )}
          </div>
          <div />
        </div>

        <div className="eyebrow" style={{ marginTop: 20, marginBottom: 4 }}>Digest</div>
        <div className="s-row">
          <div>
            <div className="label">Batch routine alerts into a summary</div>
            <div className="desc">Instead of a push for each routine item, get one digest. Urgent alerts always come through immediately.</div>
          </div>
          <div className="toolbar">
            {(["off", "daily", "weekly"] as DigestFrequency[]).map((f) => (
              <button
                key={f}
                type="button"
                className={prefs.digestFrequency === f ? "on" : ""}
                onClick={() => update({ ...prefs, digestFrequency: f })}
              >
                {f === "off" ? "Off" : f === "daily" ? "Daily" : "Weekly"}
              </button>
            ))}
          </div>
          <div />
        </div>

        <div className="eyebrow" style={{ marginTop: 20, marginBottom: 4 }}>Privacy</div>
        <div className="s-row">
          <div>
            <div className="label">Amounts in notifications</div>
            <div className="desc">Hide dollar figures and merchant names from push notifications shown on a lock screen.</div>
          </div>
          <div className="toolbar">
            {(["full", "hide_amounts"] as PrivacyLevel[]).map((level) => (
              <button
                key={level}
                type="button"
                className={prefs.privacy === level ? "on" : ""}
                onClick={() => update({ ...prefs, privacy: level })}
              >
                {level === "full" ? "Show" : "Hide"}
              </button>
            ))}
          </div>
          <div />
        </div>
      </div>
    </>
  );
}
