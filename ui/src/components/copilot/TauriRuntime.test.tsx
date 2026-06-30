import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatModelRunOptions, ChatModelRunResult, ThreadMessage } from "@assistant-ui/react";
import { createTauriChatModelAdapter } from "./TauriRuntime";

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
        payload: { type: "reasoning", conversationId: "conv-1", runId, text: "Checked budget context." },
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
        },
      });
      return { status: "ok", data: "conv-1" };
    });

    const done = vi.fn();
    const adapter = createTauriChatModelAdapter({
      getConversationId: () => "conv-1",
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
    expect(final?.metadata?.custom).toMatchObject({ modelId: "gpt-test", toolCount: 1 });
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

    const adapter = createTauriChatModelAdapter({ getConversationId: () => "conv-1" });
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

    const adapter = createTauriChatModelAdapter({ getConversationId: () => "conv-1" });
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
