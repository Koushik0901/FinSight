import { describe, expect, it } from "vitest";
import { conversationMessagesToAgUiThreadMessages } from "./TauriAgUiRuntime";
import type { ConversationMessage } from "../../../api/client";

function message(overrides: Partial<ConversationMessage>): ConversationMessage {
  return {
    id: overrides.id ?? "msg-1",
    conversationId: "conv-1",
    role: overrides.role ?? "assistant",
    content: overrides.content ?? "Hello",
    toolTrace: overrides.toolTrace ?? null,
    actionBundleId: overrides.actionBundleId ?? null,
    branchParentId: overrides.branchParentId ?? null,
    partsJson: overrides.partsJson ?? null,
    runStatus: overrides.runStatus ?? "completed",
    agUiMetadataJson: overrides.agUiMetadataJson ?? null,
    createdAt: "2026-07-01T00:00:00Z",
  };
}

describe("conversationMessagesToAgUiThreadMessages", () => {
  it("converts persisted reasoning and tool parts through AG-UI-compatible messages", () => {
    const messages = conversationMessagesToAgUiThreadMessages([
      message({ id: "u1", role: "user", content: "Check budgets" }),
      message({
        id: "a1",
        content: "Budgets look okay.",
        partsJson: JSON.stringify([
          { type: "reasoning", text: "Reviewed the budget context." },
          { type: "tool-call", toolCallId: "tool-1", toolName: "get_budgets", args: {}, argsText: "{}", result: { ok: true } },
          { type: "text", text: "Budgets look okay." },
        ]),
      }),
    ]);

    const assistant = messages.find((item) => item.id === "a1");
    expect(assistant?.content).toEqual(expect.arrayContaining([
      expect.objectContaining({ type: "tool-call", toolName: "get_budgets", result: { ok: true } }),
    ]));
  });

  it("keeps legacy rich parts when AG-UI conversion would drop finance UI artifacts", () => {
    const messages = conversationMessagesToAgUiThreadMessages([
      message({
        id: "a1",
        content: "Here is the card.",
        partsJson: JSON.stringify([
          {
            type: "generative-ui",
            id: "block-1",
            spec: { root: { component: "FinSightResponseBlock", props: { block: { kind: "callout", tone: "info", body: "Safe" } } } },
          },
          { type: "text", text: "Here is the card." },
        ]),
      }),
    ]);

    expect(messages[0]?.content).toEqual(expect.arrayContaining([
      expect.objectContaining({ type: "generative-ui", id: "block-1" }),
    ]));
  });

  it("preserves a model-authored structured block on reload (root block-mapping fix)", () => {
    // Before the fix, reasoning_result_to_agent_answer dropped the model's
    // structured blocks, so a persisted Copilot answer never carried them. Now
    // both the streaming and async-deep answers persist them. This asserts such
    // a block survives the history-load conversion as an array-content
    // generative-ui part (renderers.test.tsx covers the render itself), i.e. it
    // renders on reload instead of crashing content.map and dropping the thread.
    const messages = conversationMessagesToAgUiThreadMessages([
      message({
        id: "a1",
        content: "Your net worth is $20,606.",
        partsJson: JSON.stringify([
          {
            type: "generative-ui",
            id: "block-0",
            spec: { root: { component: "FinSightResponseBlock", props: { block: { kind: "metricGrid", metrics: [{ label: "Net worth", value: "$20,606", detail: null, tone: null }] } } } },
          },
          { type: "text", text: "Your net worth is $20,606." },
        ]),
      }),
    ]);

    const assistant = messages[0];
    expect(Array.isArray(assistant?.content)).toBe(true);
    expect(assistant?.content).toEqual(expect.arrayContaining([
      expect.objectContaining({ type: "generative-ui", id: "block-0" }),
    ]));
  });
});
