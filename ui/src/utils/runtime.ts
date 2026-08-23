/**
 * Pure self-hosted runtime helpers.
 *
 * The Tauri thin shell has been deleted (Task 4) — `isTauriRuntime()` now
 * always returns false. Kept as a stub so existing imports don't churn; new
 * code should not branch on it. `isBackendAvailable()` is true once either
 * the HTTP shim or the mock harness is installed.
 */

export function isTauriRuntime(): boolean {
  return false;
}

/**
 * True when SOME working RPC transport exists: the HTTP/SSE shim (the only
 * production transport) or the mock harness. This gates data queries.
 */
export function isBackendAvailable(): boolean {
  // In vitest/jsdom the HTTP shim is not installed (tests use the mock
  // harness via `src/test/setup.ts` or per-test `installMockBackend`), but
  // queries should still be enabled — otherwise every hook test would be
  // disabled and fail as "isSuccess false". Keep the same test-mode gate
  // the old `isTauriRuntime()` used.
  const vitest = (import.meta.env as { VITEST?: unknown }).VITEST;
  if (import.meta.env.MODE === "test" || vitest) return true;
  if (typeof navigator !== "undefined" && navigator.userAgent.includes("jsdom"))
    return true;
  if (typeof window === "undefined") return false;
  const w = window as unknown as {
    __FINSIGHT_HTTP__?: unknown;
    __FINSIGHT_MOCK__?: unknown;
  };
  return Boolean(w.__FINSIGHT_HTTP__ || w.__FINSIGHT_MOCK__);
}

export function userErrorMessage(error: unknown, fallback = "That did not work. Try again.") {
  const raw =
    error instanceof Error
      ? error.message
      : typeof error === "object" && error && "message" in error
        ? String((error as { message?: unknown }).message ?? "")
        : String(error ?? "");

  if (
    raw.includes("undefined") ||
    raw.includes("invoke") ||
    raw.includes("transformCallback") ||
    raw.includes("__TAURI")
  ) {
    return "This action needs a connected FinSight server. Reconnect and try again.";
  }

  return raw.trim() || fallback;
}
