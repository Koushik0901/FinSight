import { useEffect, useState } from "react";
import { fetchServerAbout, isClientOutdated } from "../api/serverInfo";
import { isServerMode } from "../api/auth";

/**
 * Server-mode-only "refresh to update" bar. A PWA can stay cached offline
 * after the server ships a breaking wire change; this compares the client's
 * own CLIENT_PROTOCOL against the server's `minClientProtocol`
 * (`GET /api/server/about`) and prompts a reload on mismatch.
 *
 * Desktop/Tauri builds: `isServerMode()` is false, so this renders `null`
 * synchronously with no effect run and no fetch — completely inert.
 */
export default function VersionBanner() {
  const [outdated, setOutdated] = useState(false);

  useEffect(() => {
    if (!isServerMode()) return;
    let alive = true;
    fetchServerAbout()
      .then((about) => {
        if (alive) setOutdated(isClientOutdated(about));
      })
      .catch(() => {
        // Transient network hiccup — stay quiet, not our job to surface
        // connectivity errors (AuthGate/OfflineBanner own that).
      });
    return () => {
      alive = false;
    };
  }, []);

  if (!outdated) return null;

  return (
    <div
      className="card accent version-banner"
      role="status"
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        gap: "var(--space-3)",
        borderRadius: 0,
        padding: "var(--space-2) var(--space-4)",
      }}
    >
      <span>A new version of FinSight is available.</span>
      <button type="button" className="btn primary" onClick={() => window.location.reload()}>
        Reload
      </button>
    </div>
  );
}
