import { useOnline } from "../pwa/useOnline";
import { isServerMode } from "../api/auth";

/**
 * Server-mode-only offline indicator. When the browser loses connectivity,
 * the app keeps rendering the last IndexedDB-persisted query cache (see
 * `pwa/persist.ts`) — this bar tells the user that's what they're looking at
 * and that mutations are paused until the connection returns.
 *
 * Desktop/Tauri builds: `isServerMode()` is false, so this renders `null`
 * unconditionally (the `useOnline` listeners are harmless no-ops there, but
 * the render output never changes desktop behavior).
 */
export default function OfflineBanner() {
  const online = useOnline();

  if (!isServerMode() || online) return null;

  return (
    <div
      className="card offline-banner"
      role="status"
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        gap: "var(--space-3)",
        borderRadius: 0,
        padding: "var(--space-2) var(--space-4)",
        background: "var(--surface-2)",
        color: "var(--ink-mute)",
      }}
    >
      <span>
        Offline — showing your last synced data. Changes are paused until you reconnect.
      </span>
    </div>
  );
}
