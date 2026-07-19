import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { isBackendAvailable } from "../utils/runtime";

interface ProgressPayload {
  import_id: string;
  rows_done: number;
  rows_total: number;
}

export default function ImportProgress() {
  const [active, setActive] = useState<ProgressPayload | null>(null);

  useEffect(() => {
    // The server emits import-progress/import-complete through BroadcastSink →
    // SSE, and the shim routes those frames, so this listener works in server
    // mode too; the old isTauriRuntime() gate silently dropped every frame.
    if (!isBackendAvailable()) return;
    const u1 = listen<ProgressPayload>("import-progress", (e) => setActive(e.payload));
    const u2 = listen<unknown>("import-complete", () => setActive(null));
    return () => {
      u1.then((fn) => fn());
      u2.then((fn) => fn());
    };
  }, []);

  if (!active) return null;
  const pct =
    active.rows_total === 0 ? 0 : Math.round((active.rows_done / active.rows_total) * 100);
  return (
    <div className="import-progress" role="status" aria-live="polite">
      Importing {active.rows_done.toLocaleString()} / {active.rows_total.toLocaleString()} ({pct}%)
    </div>
  );
}
