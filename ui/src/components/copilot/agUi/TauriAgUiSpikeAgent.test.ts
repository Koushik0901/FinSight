import { describe, expect, it, vi } from "vitest";
import { EventType, type BaseEvent, type RunAgentInput } from "@ag-ui/client";
import { TauriAgUiSpikeAgent } from "./TauriAgUiSpikeAgent";

function makeInput(): RunAgentInput {
  return {
    threadId: "thread-test",
    runId: "run-test",
    state: {},
    messages: [],
    tools: [],
    context: [],
    forwardedProps: {},
  };
}

describe("TauriAgUiSpikeAgent", () => {
  it("emits a minimal streamed AG-UI assistant message", async () => {
    vi.useFakeTimers();
    const agent = new TauriAgUiSpikeAgent();
    const events: BaseEvent[] = [];

    const done = new Promise<void>((resolve, reject) => {
      agent.run(makeInput()).subscribe({
        next: (event) => events.push(event),
        error: reject,
        complete: resolve,
      });
    });

    await vi.runAllTimersAsync();
    await done;
    vi.useRealTimers();

    expect(events.map((event) => event.type)).toEqual([
      EventType.RUN_STARTED,
      EventType.TEXT_MESSAGE_START,
      EventType.TEXT_MESSAGE_CONTENT,
      EventType.TEXT_MESSAGE_CONTENT,
      EventType.TEXT_MESSAGE_CONTENT,
      EventType.TEXT_MESSAGE_END,
      EventType.RUN_FINISHED,
    ]);
    expect(
      events
        .filter((event) => event.type === EventType.TEXT_MESSAGE_CONTENT)
        .map((event) => event.delta)
        .join(""),
    ).toContain("custom `AbstractAgent`");
  });
});
