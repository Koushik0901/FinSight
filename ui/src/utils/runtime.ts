type TauriWindow = Window & {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

// Tauri's IPC bridge object stays injected on ANY origin the webview navigates
// to, but Tauri's own command ACL is origin-scoped — a remote origin (e.g. the
// user's self-hosted FinSight server, once the Phase 4 desktop shell navigates
// there) gets zero command grants by default. So bridge presence alone is not
// enough to mean "use local Tauri IPC"; the page must also still be on Tauri's
// OWN internal origin. Verified against current Tauri 2 docs: macOS/Linux use
// `tauri://localhost`; Windows defaults to `http://tauri.localhost` and uses
// `https://tauri.localhost` only when `useHttpsScheme: true` is set (not set
// in this repo's tauri.conf.json, but included for robustness).
const TAURI_INTERNAL_ORIGINS = new Set([
  "tauri://localhost",
  "http://tauri.localhost",
  "https://tauri.localhost",
]);

// `pnpm tauri:dev` points the real desktop webview at Vite's dev server
// (src-tauri/tauri.conf.json's `devUrl`) for HMR, so the bridge is genuinely
// present on `http://localhost:5173` during local development. Gated on
// `DEV` (always false in a built bundle, regardless of what origin serves
// it) so a production thin-shell instance a user points at their own
// `localhost:5173` server can't false-positive into desktop-IPC mode.
const TAURI_DEV_ORIGIN = "http://localhost:5173";

export function isTauriRuntime() {
  // Access `import.meta.env.KEY` as DIRECT literal member expressions. Vite
  // statically replaces those at build; an ALIASED read
  // (`const m = import.meta; m.env.DEV`) is NOT replaced and reads a runtime
  // `import.meta.env` object that — in the dev-server browser (a real Tauri
  // webview on `pnpm tauri:dev`) — lacks DEV/MODE, so it returns undefined.
  // That silently disabled the localhost:5173 dev-origin allowance below and
  // white-screened the desktop shell (the HTTP shim then tried to install over
  // the read-only native __TAURI_INTERNALS__ and threw). Vitest keeps these
  // runtime-stubbable, so `vi.stubEnv` in the tests still works.
  const vitest = (import.meta.env as { VITEST?: unknown }).VITEST;
  if (import.meta.env.MODE === "test" || vitest) return true;
  if (typeof window === "undefined") return false;
  if (typeof navigator !== "undefined" && navigator.userAgent.includes("jsdom")) return true;
  const w = window as TauriWindow;
  // Our OWN HTTP shim also assigns `__TAURI_INTERNALS__` (that's how the
  // generated bindings keep working over HTTP), so bridge-presence alone would
  // make this predicate flip false→true the moment installHttpBackend() runs —
  // at the Vite dev origin that turned a plain `npm run dev` browser into a
  // self-reported desktop shell and let DesktopConnectGate hijack the app.
  // `__FINSIGHT_HTTP__` means "the shim is installed", i.e. definitively NOT a
  // native runtime, so check it before looking at the bridge.
  if ((w as { __FINSIGHT_HTTP__?: unknown }).__FINSIGHT_HTTP__) return false;
  if (!(w.__TAURI__ || w.__TAURI_INTERNALS__)) return false;
  if (import.meta.env.DEV && window.location.origin === TAURI_DEV_ORIGIN) return true;
  return TAURI_INTERNAL_ORIGINS.has(window.location.origin);
}

/**
 * True when SOME working RPC transport exists: a real Tauri desktop-IPC context
 * OR the HTTP/SSE shim (server, PWA, or the thin desktop shell after it has
 * navigated to a server). Use this to gate data queries/mutations — NOT
 * isTauriRuntime(), which was narrowed to origin-aware desktop detection in
 * Phase 4 and is therefore false in server mode even though RPC works over HTTP.
 */
export function isBackendAvailable(): boolean {
  if (isTauriRuntime()) return true;
  if (typeof window === "undefined") return false;
  // The HTTP shim and the design harness are both complete RPC transports.
  // Keep this predicate transport-oriented: the mock may run on 127.0.0.1,
  // localhost, or another Vite host, so origin checks would incorrectly gate
  // its queries even though __TAURI_INTERNALS__.invoke is installed.
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
    return "This action needs the desktop app runtime. Open FinSight with Tauri to use your local financial data.";
  }

  return raw.trim() || fallback;
}
