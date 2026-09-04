import { describe, expect, it } from "vitest";
import { EVENT_NAMES } from "./eventNames";

// Mirror of finsight_api::sink::event_names::ALL — if Rust adds a new event,
// re-mirror eventNames.ts (source of truth is the Rust const list; parity.rs enforces equality).
const EXPECTED_RUST_ALL = [
  "copilot-stream-frame",
  "copilot-async-answer",
  "import-progress",
  "import-complete",
  "categorization.progress",
  "categorization.complete",
  "agent.error",
  "finsight:keepalive",
  "finsight:auth-required",
] as const;

describe("eventNames", () => {
  it("covers all Rust emits (generated mirror is not stale)", () => {
    for (const name of EXPECTED_RUST_ALL) {
      expect(EVENT_NAMES).toContain(name);
    }
    expect(EVENT_NAMES.length).toBe(EXPECTED_RUST_ALL.length);
  });

  it("values are unique and non-empty", () => {
    const seen = new Set<string>();
    for (const n of EVENT_NAMES) {
      expect(n.length).toBeGreaterThan(0);
      expect(seen.has(n)).toBe(false);
      seen.add(n);
    }
  });

  it("EVENT_NAMES matches individual exports", async () => {
    const mod = await import("./eventNames");
    // Every named export should appear in EVENT_NAMES
    const named = [
      mod.COPILOT_STREAM_FRAME,
      mod.COPILOT_ASYNC_ANSWER,
      mod.IMPORT_PROGRESS,
      mod.IMPORT_COMPLETE,
      mod.CATEGORIZATION_PROGRESS,
      mod.CATEGORIZATION_COMPLETE,
      mod.AGENT_ERROR,
      mod.FINSIGHT_KEEPALIVE,
      mod.FINSIGHT_AUTH_REQUIRED,
    ];
    for (const n of named) {
      expect(EVENT_NAMES).toContain(n);
    }
  });
});
