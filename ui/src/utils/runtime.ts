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
  const meta = import.meta as { env?: { MODE?: string; VITEST?: string; DEV?: boolean } };
  if (meta.env?.MODE === "test" || meta.env?.VITEST) return true;
  if (typeof window === "undefined") return false;
  if (typeof navigator !== "undefined" && navigator.userAgent.includes("jsdom")) return true;
  const w = window as TauriWindow;
  if (!(w.__TAURI__ || w.__TAURI_INTERNALS__)) return false;
  if (meta.env?.DEV && window.location.origin === TAURI_DEV_ORIGIN) return true;
  return TAURI_INTERNAL_ORIGINS.has(window.location.origin);
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
