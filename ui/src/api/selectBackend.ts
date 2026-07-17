import { isTauriRuntime } from "../utils/runtime";

/**
 * Decide which client transport `main.tsx`'s `boot()` should install.
 * Extracted into its own module so the bridge/origin decision is unit-testable
 * without running `main.tsx`'s full render tree.
 *
 * - `"mock"` — DEV-only `?mock=…` design harness (plain browser, no real Tauri
 *   bridge): a fixture-backed `__TAURI_INTERNALS__` so the app renders sample
 *   data with no backend.
 * - `"http"` — the production HTTP/SSE shim (`installHttpBackend`): the real
 *   transport for the browser, the installed PWA, AND the thin desktop shell
 *   once it has navigated to a remote server.
 * - `"none"` — a real desktop-IPC context on Tauri's own origin: leave the
 *   native `__TAURI_INTERNALS__` bridge in place (the shell's pre-navigation
 *   ConnectScreen phase talks to the 3 local config commands through it).
 */
export function selectBackend(params: URLSearchParams): "mock" | "http" | "none" {
  if (typeof window === "undefined") return "none";
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  // DEV-only ?mock design harness — only ever used in a plain browser, so it
  // deliberately keys on raw bridge-absence (don't shadow a real Tauri runtime).
  if (import.meta.env.DEV && params.has("mock") && !w.__TAURI_INTERNALS__) return "mock";
  // Install the HTTP shim whenever we're NOT in a real desktop-IPC context.
  // CRITICAL: this is isTauriRuntime() (origin-aware), NOT raw bridge presence.
  // The thin desktop shell keeps __TAURI_INTERNALS__ injected after it navigates
  // to a remote server (Tauri injects the bridge on every origin), but at that
  // remote origin Tauri's command ACL is empty — so the app MUST switch to the
  // HTTP transport there. Gating on raw `!__TAURI_INTERNALS__` (as this did
  // before Phase 4's thin shell existed) would leave the navigated shell talking
  // to a dead local bridge and every RPC would fail.
  if (!isTauriRuntime()) return "http";
  return "none";
}
