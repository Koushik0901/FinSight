import { useEffect, useState } from "react";

/**
 * Tracks `navigator.onLine`, updated via the `online`/`offline` window
 * events. SSR-safe: defaults to `true` when `navigator` isn't available.
 */
export function useOnline(): boolean {
  const [online, setOnline] = useState(
    typeof navigator === "undefined" ? true : navigator.onLine
  );

  useEffect(() => {
    const goOnline = () => setOnline(true);
    const goOffline = () => setOnline(false);
    window.addEventListener("online", goOnline);
    window.addEventListener("offline", goOffline);
    return () => {
      window.removeEventListener("online", goOnline);
      window.removeEventListener("offline", goOffline);
    };
  }, []);

  return online;
}

export default useOnline;
