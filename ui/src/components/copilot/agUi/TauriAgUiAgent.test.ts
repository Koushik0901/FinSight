import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BaseEvent, RunAgentInput } from "@ag-ui/client";
import { TauriAgUiAgent } from "./TauriAgUiAgent";

type Listener = (event: { payload: unknown }) => void;

const listeners: Record<string, Listener> = {};

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, cb: Listener) => {
    listeners[name] = cb;
    return () => {
      delete listeners[name];
    };
  }),
}));

vi.mock("../../../api/client", () => ({
  commands: {
    createConversation: vi.fn(async () => ({ status: "ok", data: "conv-1" })),
    streamCopilotMessage: vi.fn(async () => ({ status: "ok", data: "conv-1" })),
  },
}));

function input(text = "Plan my budget"): RunAgentInput {
  return {
    threadId: "thread-1",
    runId: "run-1",
    state: {},
    messages: [
      {
        id: "user-1",
        role: "user",
        content: [{ type: "text", text }],
      },
    ],
    tools: [],
    context: [],
    forwardedProps: {},
  } as RunAgentInput;
}

function emitCopilotFrame(payload: unknown) {
  const listener = listeners["copilot-stream-frame"];
  if (!listener) throw new Error("copilot-stream-frame listener was not registered");
  listener({ payload });
}

describe("TauriAgUiAgent", () => {
  beforeEach(() => {
    for (const key of Object.keys(listeners)) delete listeners[key];
    vi.clearAllMocks();
  });

  it("maps Copilot stream frames into AG-UI lifecycle, reasoning, tool, and text events", async () => {
    const agent = new TauriAgUiAgent();
    const events: BaseEvent[] = [];
    const done = new Promise<void>((resolve, reject) => {
      agent.run(input()).subscribe({
        next: (event) => events.push(event),
        error: reject,
        complete: resolve,
      });
    });

    await vi.waitFor(() => expect(listeners["copilot-stream-frame"]).toBeTypeOf("function"));
    emitCopilotFrame({ type: "reasoning", conversationId: "conv-1", runId: "run-1", sequenceNumber: 0, text: "Checked budgets." });
    emitCopilotFrame({ type: "toolCallStart", conversationId: "conv-1", runId: "run-1", sequenceNumber: 1, toolCallId: "tool-1", toolName: "get_budgets", args: { month: "2026-07" } });
    emitCopilotFrame({ type: "toolCallResult", conversationId: "conv-1", runId: "run-1", sequenceNumber: 2, toolCallId: "tool-1", result: { ok: true }, isError: false });
    emitCopilotFrame({ type: "text", conversationId: "conv-1", runId: "run-1", sequenceNumber: 3, delta: "Here is " });
    emitCopilotFrame({ type: "text", conversationId: "conv-1", runId: "run-1", sequenceNumber: 4, delta: "the plan." });
    emitCopilotFrame({ type: "done", conversationId: "conv-1", runId: "run-1", sequenceNumber: 5, messageId: "asst-1", bundleId: null, toolTrace: [], followUpQuestions: [], actionLabel: null, actionPath: null, providerId: "test", modelId: "test", elapsedMs: 10, toolCount: 1 });

    await done;

    expect(events.map((event) => event.type)).toEqual([
      "RUN_STARTED",
      "REASONING_START",
      "REASONING_MESSAGE_START",
      "REASONING_MESSAGE_CONTENT",
      "TOOL_CALL_START",
      "TOOL_CALL_ARGS",
      "TOOL_CALL_END",
      "TOOL_CALL_RESULT",
      "TEXT_MESSAGE_START",
      "TEXT_MESSAGE_CONTENT",
      "TEXT_MESSAGE_CONTENT",
      "REASONING_MESSAGE_END",
      "REASONING_END",
      "TEXT_MESSAGE_END",
      "RUN_FINISHED",
    ]);
  });

  it("ignores stale frames from other runs", async () => {
    const agent = new TauriAgUiAgent();
    const events: BaseEvent[] = [];
    const done = new Promise<void>((resolve, reject) => {
      agent.run(input()).subscribe({ next: (event) => events.push(event), error: reject, complete: resolve });
    });

    await vi.waitFor(() => expect(listeners["copilot-stream-frame"]).toBeTypeOf("function"));
    emitCopilotFrame({ type: "text", conversationId: "conv-1", runId: "old-run", sequenceNumber: 0, delta: "stale" });
    emitCopilotFrame({ type: "done", conversationId: "conv-1", runId: "run-1", sequenceNumber: 0, messageId: "asst-1", bundleId: null, toolTrace: [], followUpQuestions: [], actionLabel: null, actionPath: null, providerId: "test", modelId: "test", elapsedMs: 10, toolCount: 0 });

    await done;

    expect(events.some((event) => event.type === "TEXT_MESSAGE_CONTENT" && "delta" in event && event.delta === "stale")).toBe(false);
  });

  it("fails safely on out-of-order sequence gaps", async () => {
    const agent = new TauriAgUiAgent();
    const events: BaseEvent[] = [];
    const done = new Promise<void>((resolve, reject) => {
      agent.run(input()).subscribe({ next: (event) => events.push(event), error: reject, complete: resolve });
    });

    await vi.waitFor(() => expect(listeners["copilot-stream-frame"]).toBeTypeOf("function"));
    emitCopilotFrame({ type: "text", conversationId: "conv-1", runId: "run-1", sequenceNumber: 0, delta: "first" });
    emitCopilotFrame({ type: "text", conversationId: "conv-1", runId: "run-1", sequenceNumber: 2, delta: "gap" });

    await done;

    expect(events.at(-1)?.type).toBe("RUN_ERROR");
  });

  it("emits a backend-issued approval tool call when a run returns an action bundle", async () => {
    const agent = new TauriAgUiAgent();
    const events: BaseEvent[] = [];
    const done = new Promise<void>((resolve, reject) => {
      agent.run(input()).subscribe({ next: (event) => events.push(event), error: reject, complete: resolve });
    });

    await vi.waitFor(() => expect(listeners["copilot-stream-frame"]).toBeTypeOf("function"));
    emitCopilotFrame({ type: "text", conversationId: "conv-1", runId: "run-1", sequenceNumber: 0, delta: "Review these actions." });
    emitCopilotFrame({ type: "done", conversationId: "conv-1", runId: "run-1", sequenceNumber: 1, messageId: "asst-1", bundleId: "bundle-1", toolTrace: [], followUpQuestions: [], actionLabel: null, actionPath: null, providerId: "test", modelId: "test", elapsedMs: 10, toolCount: 0 });

    await done;

    const approvalStart = events.find((event) => event.type === "TOOL_CALL_START" && "toolCallName" in event && event.toolCallName === "request_action_approval");
    const approvalResult = events.find((event) => event.type === "TOOL_CALL_RESULT" && "toolCallId" in event && event.toolCallId === "approval-bundle-1");
    expect(approvalStart).toBeTruthy();
    const approvalContent = approvalResult && "content" in approvalResult && typeof approvalResult.content === "string"
      ? JSON.parse(approvalResult.content) as unknown
      : null;
    expect(approvalContent).toMatchObject({
      kind: "approval_request",
      bundleId: "bundle-1",
      source: "backend",
    });
  });

  it("surfaces a structured command error's message, never [object Object]", async () => {
    const { commands } = await import("../../../api/client");
    (commands.streamCopilotMessage as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      status: "error",
      error: { code: "agent.reasoning", message: "The Copilot could not complete this request." },
    });

    const agent = new TauriAgUiAgent();
    const events: BaseEvent[] = [];
    const done = new Promise<void>((resolve, reject) => {
      agent.run(input()).subscribe({ next: (event) => events.push(event), error: reject, complete: resolve });
    });
    await done;

    const runError = events.find((event) => event.type === "RUN_ERROR");
    expect(runError && "message" in runError ? runError.message : "").toBe(
      "The Copilot could not complete this request.",
    );
    const shownText = events
      .filter((event) => event.type === "TEXT_MESSAGE_CONTENT")
      .map((event) => ("delta" in event ? String(event.delta) : ""))
      .join("");
    expect(shownText).not.toContain("[object Object]");
  });
});
