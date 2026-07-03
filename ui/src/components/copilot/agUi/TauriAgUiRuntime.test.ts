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
});
