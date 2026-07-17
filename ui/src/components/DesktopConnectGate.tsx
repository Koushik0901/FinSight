import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "../utils/runtime";
import ConnectScreen from "../screens/desktop/ConnectScreen";

/** Only relevant to the bundled desktop shell (isTauriRuntime() — which is
 *  now origin-aware, see Phase 4 Task 6 — so this correctly stops rendering
 *  once the window has navigated to a real server, since at that point the
 *  page is no longer on Tauri's internal origin). Elsewhere (browser, PWA,
 *  post-navigate shell) this renders children immediately with zero effect.
 *
 *  Navigation note: the plan's original sketch called
 *  `getCurrentWebviewWindow().navigate(url)`, but that method does not exist
 *  on `@tauri-apps/api`'s `WebviewWindow` (verified against the installed
 *  package and Tauri's own docs — Rust's `WebviewWindow::navigate()` exists,
 *  there is no JS-side equivalent). Plain `window.location.href = url` is the
 *  standard web-platform way to navigate the current webview, and it's
 *  exactly the mechanism Task 6's origin-aware `isTauriRuntime()` already
 *  assumes is possible. Assigning `location.href` unloads the current JS
 *  context synchronously, so there is no meaningful way to `.catch()` a
 *  failed navigation the way a promise-returning `.navigate()` would allow —
 *  a bad/unreachable URL simply fails to load as an ordinary browser
 *  navigation failure, not a JS exception. */
export default function DesktopConnectGate({ children }: { children: ReactNode }) {
  const [state, setState] = useState<"checking" | "needsConnect" | "connecting">("checking");

  useEffect(() => {
    if (!isTauriRuntime()) { setState("connecting"); return; }
    let alive = true;
    invoke<string | null>("get_server_url").then((url) => {
      if (!alive) return;
      if (url) {
        setState("connecting");
        window.location.href = url;
      } else {
        setState("needsConnect");
      }
    }).catch(() => { if (alive) setState("needsConnect"); });
    return () => { alive = false; };
  }, []);

  if (!isTauriRuntime() || state === "connecting") return <>{children}</>;
  if (state === "checking") return null; // avoid a flash of ConnectScreen while checking
  return <ConnectScreen onConnected={(url) => { window.location.href = url; }} />;
}
