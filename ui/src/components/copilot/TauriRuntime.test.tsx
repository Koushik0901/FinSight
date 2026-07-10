import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatModelRunOptions, ChatModelRunResult, ThreadMessage } from "@assistant-ui/react";
import { buildMetaFromMessages, createTauriChatModelAdapter } from "./TauriRuntime";
import type { ConversationMessage } from "../../api/client";

const eventMocks = vi.hoisted(() => ({
  listen: vi.fn(),
}));

const commandMocks = vi.hoisted(() => ({
  streamCopilotMessage: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: eventMocks.listen,
}));

vi.mock("../../api/client", () => ({
  commands: {
    streamCopilotMessage: commandMocks.streamCopilotMessage,
  },
}));

function makeRunOptions(messages: ThreadMessage[]): ChatModelRunOptions {
  const abortController = new AbortController();
  return {
    messages,
    abortSignal: abortController.signal,
    runConfig: {},
    context: {
      system: "",
      tools: [],
      callSettings: {},
      config: {},
    },
    unstable_getMessage: () => messages[messages.length - 1],
  } as unknown as ChatModelRunOptions;
}

function userMessage(id: string, text: string): ThreadMessage {
  return {
    id,
    role: "user",
    content: [{ type: "text", text }],
    attachments: [],
    metadata: { custom: {} },
    createdAt: new Date(),
  } as unknown as ThreadMessage;
}

function assistantMessage(id: string, text: string): ThreadMessage {
  return {
    id,
    role: "assistant",
    content: [{ type: "text", text }],
    status: { type: "complete", reason: "stop" },
    metadata: { custom: {} },
    createdAt: new Date(),
  } as unknown as ThreadMessage;
}

describe("createTauriChatModelAdapter", () => {
  beforeEach(() => {
    eventMocks.listen.mockReset();
    commandMocks.streamCopilotMessage.mockReset();
  });

  it("streams correlated Tauri frame events into assistant-ui rich parts", async () => {
    const listeners: Record<string, (event: { payload: unknown }) => void> = {};
    eventMocks.listen.mockImplementation(async (name: string, cb: (event: { payload: unknown }) => void) => {
      listeners[name] = cb;
      return vi.fn();
    });
    commandMocks.streamCopilotMessage.mockImplementation(async (_conversationId: string, runId: string) => {
      listeners["copilot-stream-frame"]?.({
        payload: { type: "text", conversationId: "conv-1", runId: "stale-run", delta: "ignored " },
      });
      listeners["copilot-stream-frame"]?.({
        payload: { type: "reasoning", conversation_id: "conv-1", run_id: runId, text: "Checked budget context." },
      });
      listeners["copilot-stream-frame"]?.({
        payload: { type: "toolCallStart", conversationId: "conv-1", runId, toolCallId: "tool-1", toolName: "get_budgets", args: {} },
      });
      listeners["copilot-stream-frame"]?.({
        payload: { type: "toolCallResult", conversationId: "conv-1", runId, toolCallId: "tool-1", result: { ok: true }, isError: false },
      });
      listeners["copilot-stream-frame"]?.({
        payload: { type: "text", conversationId: "conv-1", runId, delta: "Hello " },
      });
      listeners["copilot-stream-frame"]?.({
        payload: { type: "text", conversationId: "conv-1", runId, delta: "there" },
      });
      listeners["copilot-stream-frame"]?.({
        payload: { type: "usage", conversationId: "conv-1", runId, providerId: "openai", modelId: "gpt-test", elapsedMs: 1200, toolCount: 1 },
      });
      listeners["copilot-stream-frame"]?.({
        payload: {
          type: "done",
          conversationId: "conv-1",
          runId,
          messageId: "assistant-db-id",
          bundleId: null,
          toolTrace: [],
          followUpQuestions: [],
          actionLabel: null,
          actionPath: null,
          providerId: "openai",
          modelId: "gpt-test",
          elapsedMs: 1200,
          toolCount: 1,
          cachedTokens: 800,
          promptTokens: 1000,
        },
      });
      return { status: "ok", data: "conv-1" };
    });

    const done = vi.fn();
    const adapter = createTauriChatModelAdapter({
      ensureConversationId: async () => "conv-1",
      onDone: done,
    });

    const chunks: ChatModelRunResult[] = [];
    for await (const chunk of adapter.run(
      makeRunOptions([userMessage("u1", "Hi")])
    ) as AsyncGenerator<ChatModelRunResult, void>) {
      chunks.push(chunk);
    }

    expect(commandMocks.streamCopilotMessage).toHaveBeenCalledWith(
      "conv-1",
      expect.any(String),
      "Hi",
      [],
      null
    );
    const final = chunks.at(-1);
    expect(final?.content?.some((part) => part.type === "reasoning")).toBe(true);
    expect(final?.content?.some((part) => part.type === "tool-call")).toBe(true);
    expect(final?.content?.find((part) => part.type === "text")).toMatchObject({
      type: "text",
      text: "Hello there",
    });
    expect(final?.metadata?.custom).toMatchObject({ modelId: "gpt-test", toolCount: 1, cachedTokens: 800, promptTokens: 1000 });
    expect(done).toHaveBeenCalledWith(
      expect.objectContaining({ messageId: "assistant-db-id" }),
      expect.objectContaining({})
    );
  });

  it("passes previous user and assistant turns as backend history", async () => {
    eventMocks.listen.mockResolvedValue(vi.fn());
    commandMocks.streamCopilotMessage.mockImplementation(async (_conversationId: string, runId: string) => {
      const done = eventMocks.listen.mock.calls.find(([name]) => name === "copilot-stream-frame")?.[1];
      done({
        payload: {
          type: "done",
          conversationId: "conv-1",
          runId,
          messageId: "assistant-db-id",
          bundleId: null,
          toolTrace: [],
          followUpQuestions: [],
          actionLabel: null,
          actionPath: null,
          providerId: "openai",
          modelId: "gpt-test",
          elapsedMs: 20,
          toolCount: 0,
        },
      });
      return { status: "ok", data: "conv-1" };
    });

    const adapter = createTauriChatModelAdapter({ ensureConversationId: async () => "conv-1" });
    for await (const _ of adapter.run(
      makeRunOptions([
        userMessage("u1", "First"),
        assistantMessage("a1", "Answer"),
        userMessage("u2", "Follow-up"),
      ])
    ) as AsyncGenerator<ChatModelRunResult, void>) {
      // consume stream
    }

    expect(commandMocks.streamCopilotMessage).toHaveBeenCalledWith(
      "conv-1",
      expect.any(String),
      "Follow-up",
      [
        { role: "user", content: "First" },
        { role: "assistant", content: "Answer" },
      ],
      null
    );
  });

  it("turns command failures into assistant error content", async () => {
    eventMocks.listen.mockResolvedValue(vi.fn());
    commandMocks.streamCopilotMessage.mockResolvedValue({
      status: "error",
      error: { code: "agent.empty_response", message: "empty" },
    });

    const adapter = createTauriChatModelAdapter({ ensureConversationId: async () => "conv-1" });
    const chunks: ChatModelRunResult[] = [];
    for await (const chunk of adapter.run(
      makeRunOptions([userMessage("u1", "Clean up uncategorized transactions")])
    ) as AsyncGenerator<ChatModelRunResult, void>) {
      chunks.push(chunk);
    }

    expect(chunks).toEqual([
      {
        content: [
          {
            type: "text",
            text: "Copilot finished without a text response. Check the configured AI provider/model in Settings -> Agent, then try again.",
          },
        ],
        status: {
          type: "incomplete",
          reason: "error",
          error: "agent.empty_response: empty",
        },
      },
    ]);
  });
});

describe("buildMetaFromMessages — cache-usage reload survival", () => {
  it("recovers cachedTokens/promptTokens from persisted agUiMetadataJson", () => {
    // The live cache chip rides the Usage/Done frames; on reload it must come
    // back from agUiMetadataJson alongside elapsedMs/toolCount, or the chip
    // vanishes when the thread is reopened.
    const meta = buildMetaFromMessages([
      {
        id: "a1",
        conversationId: "conv-1",
        role: "assistant",
        content: "Answer.",
        toolTrace: null,
        actionBundleId: null,
        branchParentId: null,
        partsJson: null,
        runStatus: "completed",
        agUiMetadataJson: JSON.stringify({
          schemaVersion: 1,
          elapsedMs: 1200,
          toolCount: 2,
          cachedTokens: 800,
          promptTokens: 1000,
        }),
        createdAt: "2026-07-01T00:00:00Z",
      } as ConversationMessage,
    ]);
    expect(meta["a1"]).toMatchObject({ cachedTokens: 800, promptTokens: 1000, toolCount: 2 });
  });

  it("recovers a full deep-answer meta footer (model + timing + cache, no bundle)", () => {
    // spawn_deep_answer now persists agUiMetadataJson in exactly this shape, so a
    // background "deep answer" gets the same meta footer — including the cache
    // chip — on reload, instead of a bare bubble with no model/timing/tokens.
    const meta = buildMetaFromMessages([
      {
        id: "a1",
        conversationId: "conv-1",
        role: "assistant",
        content: "Deeper analysis.",
        toolTrace: null,
        actionBundleId: null,
        branchParentId: null,
        partsJson: null,
        runStatus: "completed",
        agUiMetadataJson: JSON.stringify({
          schemaVersion: 1,
          runtime: "ag-ui",
          runStatus: "completed",
          providerId: "openrouter",
          modelId: "google/gemini-2.5-flash",
          elapsedMs: 9000,
          toolCount: 5,
          cachedTokens: 1200,
          promptTokens: 3000,
          toolTrace: ["Called tool: get_liabilities"],
          plan: ["Assess debt", "Compare options"],
          followUpQuestions: ["Want a payoff plan?"],
        }),
        createdAt: "2026-07-01T00:00:00Z",
      } as ConversationMessage,
    ]);
    expect(meta["a1"]).toMatchObject({
      providerId: "openrouter",
      modelId: "google/gemini-2.5-flash",
      elapsedMs: 9000,
      toolCount: 5,
      cachedTokens: 1200,
      promptTokens: 3000,
    });
  });
});
