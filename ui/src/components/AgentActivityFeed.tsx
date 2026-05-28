import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

type Progress = { done: number; total: number };
type Complete = { categorized: number; skipped: number };

type FeedState =
  | { kind: "idle" }
  | { kind: "progress"; done: number; total: number }
  | { kind: "done"; categorized: number };

export default function AgentActivityFeed() {
  const [state, setState] = useState<FeedState>({ kind: "idle" });
  const fadeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    let cancelled = false;
    const pending: Array<() => void> = [];

    listen<Progress>("categorization.progress", (e) => {
      setState({ kind: "progress", done: e.payload.done, total: e.payload.total });
      if (fadeTimer.current) clearTimeout(fadeTimer.current);
    }).then((fn) => {
      if (cancelled) fn(); // already unmounted — call unlisten immediately
      else pending.push(fn);
    });

    listen<Complete>("categorization.complete", (e) => {
      setState({ kind: "done", categorized: e.payload.categorized });
      fadeTimer.current = setTimeout(() => setState({ kind: "idle" }), 3000);
    }).then((fn) => {
      if (cancelled) fn(); // already unmounted — call unlisten immediately
      else pending.push(fn);
    });

    return () => {
      cancelled = true;
      pending.forEach((fn) => fn());
      if (fadeTimer.current) clearTimeout(fadeTimer.current);
    };
  }, []);

  return (
    <div
      aria-live="polite"
      aria-atomic="true"
      style={{ minHeight: 20, fontSize: 13, color: "var(--text-2)", marginBottom: 8 }}
    >
      {state.kind === "progress" && (
        <span>Categorizing… {state.done} / {state.total}</span>
      )}
      {state.kind === "done" && (
        <span>Done — {state.categorized} transactions categorized</span>
      )}
    </div>
  );
}
