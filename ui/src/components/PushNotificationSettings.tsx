import { useEffect, useState } from "react";
import { toast } from "sonner";
import {
  usePushStatus,
  useSavePushSubscription,
  useDeletePushSubscription,
  useSendTestPush,
} from "../api/hooks/push";
import {
  currentSubscription,
  disablePush,
  enablePush,
  notificationPermission,
  pushSupported,
} from "../pwa/push";
import { isServerMode } from "../api/auth";

/**
 * Per-device Web Push opt-in, for the Settings > Notifications section.
 *
 * Subscription state is DEVICE-local, not account-level: the same account can
 * be pushed on a phone and not on a desktop. So the toggle reflects what this
 * browser has registered (`currentSubscription()`), while the count beside it
 * comes from the server and covers every device.
 */
export default function PushNotificationSettings() {
  const { data: status } = usePushStatus();
  const save = useSavePushSubscription();
  const remove = useDeletePushSubscription();
  const test = useSendTestPush();

  // null = still checking this device; avoids flashing "off" before we know.
  const [subscribed, setSubscribed] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void currentSubscription().then((sub) => {
      if (!cancelled) setSubscribed(Boolean(sub));
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // Push needs a service worker and an authenticated server to send from —
  // neither exists in the Tauri shell or a non-secure origin.
  if (!isServerMode() || !pushSupported()) {
    return (
      <div className="s-row">
        <div>
          <div className="label">Push notifications</div>
          <div className="desc">
            Alerts that reach you with FinSight closed. Needs the installed web app over
            HTTPS — this browser or runtime doesn&apos;t support them.
          </div>
        </div>
        <div className="muted">Unavailable</div>
        <div />
      </div>
    );
  }

  const permission = notificationPermission();
  const blocked = permission === "denied";

  async function toggle(next: boolean) {
    if (busy) return;
    setBusy(true);
    try {
      if (next) {
        const result = await enablePush(status?.publicKey ?? "");
        if (!result.ok) {
          toast.error(
            result.reason === "denied"
              ? "Notifications were blocked"
              : "Couldn't enable notifications",
            {
              description:
                result.reason === "denied"
                  ? "Allow notifications for this site in your browser settings, then try again."
                  : "This device couldn't register for push.",
            }
          );
          return;
        }
        await save.mutateAsync({ ...result.payload, label: deviceLabel() });
        setSubscribed(true);
        toast.success("Notifications on for this device");
      } else {
        const endpoint = await disablePush();
        // Always tell the server, even if the browser had already dropped the
        // subscription — otherwise its row lingers and every send retries a
        // device that will never answer.
        if (endpoint) await remove.mutateAsync(endpoint);
        setSubscribed(false);
        toast.success("Notifications off for this device");
      }
    } catch (err) {
      toast.error("Couldn't update notifications", { description: String(err) });
    } finally {
      setBusy(false);
    }
  }

  async function sendTest() {
    try {
      const report = await test.mutateAsync();
      if (report.delivered > 0) {
        toast.success(`Sent to ${report.delivered} device${report.delivered === 1 ? "" : "s"}`, {
          description: "If nothing appears, check notification permissions for this site.",
        });
      } else if (report.expired > 0) {
        toast.error("Those devices are no longer reachable", {
          description: "Their subscriptions expired and have been removed. Turn the toggle back on.",
        });
      } else {
        toast.error("Nothing was delivered", {
          description: report.failed > 0 ? "The push service rejected the message." : "No devices registered yet.",
        });
      }
    } catch (err) {
      toast.error("Couldn't send the test", { description: String(err) });
    }
  }

  const deviceCount = status?.deviceCount ?? 0;

  return (
    <>
      <div className="s-row">
        <div>
          <div className="label">Push notifications</div>
          <div className="desc">
            {blocked
              ? "Blocked in this browser. Allow notifications for this site to turn them on."
              : "Reach this device even when FinSight is closed. Registered per device."}
          </div>
        </div>
        <div className="muted">
          {deviceCount > 0 ? `${deviceCount} device${deviceCount === 1 ? "" : "s"}` : "No devices"}
        </div>
        <span
          className={`tog${subscribed ? " on" : ""}`}
          role="switch"
          aria-checked={Boolean(subscribed)}
          aria-label="Push notifications on this device"
          aria-disabled={busy || blocked}
          tabIndex={0}
          onClick={() => !blocked && void toggle(!subscribed)}
          onKeyDown={(e) => e.key === "Enter" && !blocked && void toggle(!subscribed)}
        />
      </div>

      {subscribed && (
        <div className="s-row">
          <div>
            <div className="label">Test notification</div>
            <div className="desc">
              Confirm the whole chain works on this device — permission, subscription, and delivery.
            </div>
          </div>
          <div />
          <button
            type="button"
            className="btn sm"
            onClick={() => void sendTest()}
            disabled={test.isPending}
          >
            {test.isPending ? "Sending…" : "Send test"}
          </button>
        </div>
      )}
    </>
  );
}

/**
 * A short, non-identifying hint so a user can tell their devices apart when
 * revoking one. Deliberately coarse — the platform token only, never the full
 * user-agent string, which is a fingerprinting surface we have no use for.
 */
function deviceLabel(): string {
  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  if (/android/i.test(ua)) return "Android";
  if (/iphone|ipad|ipod/i.test(ua)) return "iOS";
  if (/mac os x/i.test(ua)) return "Mac";
  if (/windows/i.test(ua)) return "Windows";
  if (/linux/i.test(ua)) return "Linux";
  return "This device";
}
