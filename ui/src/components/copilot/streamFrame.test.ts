import { describe, expect, it } from "vitest";
import { normalizeCopilotStreamFrame } from "./streamFrame";

const base = {
  type: "text",
  conversationId: "c1",
  runId: "r1",
};

describe("normalizeCopilotStreamFrame", () => {
  it("rejects non-objects and frames missing conversation/run ids", () => {
    expect(normalizeCopilotStreamFrame(null, "agui")).toBeNull();
    expect(normalizeCopilotStreamFrame("nope", "legacy")).toBeNull();
    expect(normalizeCopilotStreamFrame({ type: "text" }, "agui")).toBeNull();
    expect(normalizeCopilotStreamFrame({ ...base, conversationId: "" }, "agui")).toBeNull();
  });

  it("maps snake_case aliases onto camelCase frame fields", () => {
    const frame = normalizeCopilotStreamFrame(
      {
        type: "tool_call_start",
        conversation_id: "c1",
        run_id: "r1",
        tool_call_id: "t1",
        tool_name: "search_transactions",
        args: { query: "coffee" },
      },
      "agui",
    );
    expect(frame).toMatchObject({
      type: "toolCallStart",
      toolCallId: "t1",
      toolName: "search_transactions",
      args: { query: "coffee" },
    });
  });

  it("drops live plan frames in the legacy variant and surfaces them in agui", () => {
    const payload = {
      type: "plan",
      conversationId: "c1",
      runId: "r1",
      steps: ["a", "b"],
    };
    expect(normalizeCopilotStreamFrame(payload, "legacy")).toBeNull();
    expect(normalizeCopilotStreamFrame(payload, "agui")).toEqual({
      type: "plan",
      conversationId: "c1",
      runId: "r1",
      threadId: undefined,
      assistantMessageId: undefined,
      parentMessageId: null,
      sequenceNumber: -1,
      steps: ["a", "b"],
    });
  });

  it("coerces missing optional ids to empty strings only in the legacy variant", () => {
    const payload = { ...base };
    const legacy = normalizeCopilotStreamFrame(payload, "legacy");
    const agui = normalizeCopilotStreamFrame(payload, "agui");
    expect(legacy).toMatchObject({ threadId: "", assistantMessageId: "", delta: "" });
    expect(agui).toMatchObject({ threadId: undefined, assistantMessageId: undefined });
  });

  it("parses usage/done/error payloads identically except for the documented id defaulting", () => {
    const usage = { type: "usage", conversationId: "c1", runId: "r1", elapsed_ms: 12, prompt_tokens: 5 };
    const legacyUsage = normalizeCopilotStreamFrame(usage, "legacy");
    const aguiUsage = normalizeCopilotStreamFrame(usage, "agui");
    // The only allowed difference: missing optional ids.
    expect(legacyUsage).toMatchObject({ type: "usage", threadId: "", elapsedMs: 12 });
    expect(aguiUsage).toMatchObject({ type: "usage", threadId: undefined, elapsedMs: 12 });
    expect({ ...legacyUsage, threadId: undefined, assistantMessageId: undefined }).toEqual(aguiUsage);

    const error = { type: "error", conversationId: "c1", runId: "r1", code: "copilot.x", message: "boom" };
    const legacyError = normalizeCopilotStreamFrame(error, "legacy");
    const aguiError = normalizeCopilotStreamFrame(error, "agui");
    expect(legacyError).toMatchObject({ type: "error", code: "copilot.x", message: "boom" });
    expect({ ...legacyError, threadId: undefined, assistantMessageId: undefined }).toEqual(aguiError);
  });
});
