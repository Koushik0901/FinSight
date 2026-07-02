type TauriWindow = Window & {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

export function isTauriRuntime() {
  const meta = import.meta as { env?: { MODE?: string; VITEST?: string } };
  if (meta.env?.MODE === "test" || meta.env?.VITEST) return true;
  if (typeof window === "undefined") return false;
  if (typeof navigator !== "undefined" && navigator.userAgent.includes("jsdom")) return true;
  const w = window as TauriWindow;
  return Boolean(w.__TAURI__ || w.__TAURI_INTERNALS__);
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
