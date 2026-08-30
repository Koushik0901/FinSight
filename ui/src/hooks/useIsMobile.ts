import { useEffect, useState } from "react";

/**
 * Semantic mobile breakpoint — 768px.
 * Phone = 0-768, Tablet/Desktop = 769+.
 * We avoid the old 900px hybrid where a tablet got a "big phone" layout.
 * Uses matchMedia so JS and CSS agree on a single source of truth.
 */
const MOBILE_QUERY = "(max-width: 768px)";

function getIsMobile(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia(MOBILE_QUERY).matches;
}

export function useIsMobile(): boolean {
  const [isMobile, setIsMobile] = useState(getIsMobile);

  useEffect(() => {
    const mql = window.matchMedia(MOBILE_QUERY);
    const onChange = (e: MediaQueryListEvent) => setIsMobile(e.matches);
    // Sync once on mount in case SSR or resize happened.
    setIsMobile(mql.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  return isMobile;
}

/** Exported for tests to assert the breakpoint value without hard-coding 768 twice. */
export const MOBILE_BREAKPOINT = 768;
export const MOBILE_MEDIA_QUERY = MOBILE_QUERY;
