import { listen } from "@tauri-apps/api/event";
import { AbstractAgent, type BaseEvent, type RunAgentInput } from "@ag-ui/client";
import { Observable } from "rxjs";
import { commands } from "../../../api/client";
import type { ChatHistoryEntry, CopilotStreamFrame } from "../../../api/client";
import { serializeFinanceArtifactEnvelope } from "./artifacts";

function unwrapCommandResult<T>(result: { status: "ok"; data: T } | { status: "error"; error: unknown }): T {
  if (result.status === "error") throw result.error;
  return result.data;
}

function createId(prefix: string) {
  const uuid = globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2);
  return `${prefix}-${uuid}`;
}

/// Extract a UI-safe message from a backend command error. The backend returns
/// a structured `{ code, message }` AppError; we surface only its `message`
/// (already sanitized server-side) and never `String(err)` — which would render
/// "[object Object]" or risk echoing raw internals.
function safeCommandErrorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message;
  }
  return "The Copilot request failed. Please try again.";
}

function textFromAgUiMessage(message: unknown): string {
  if (!message || typeof message !== "object") return "";
  const content = (message as { content?: unknown }).content;
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .flatMap((part) => {
      if (!part || typeof part !== "object") return [];
      const record = part as Record<string, unknown>;
      if (record.type === "text" && typeof record.text === "string") return [record.text];
      return [];
    })
    .join("\n");
}

function historyFromAgUiMessages(messages: readonly unknown[]): ChatHistoryEntry[] {
  return messages.slice(0, -1).flatMap((message) => {
    if (!message || typeof message !== "object") return [];
    const role = (message as { role?: unknown }).role;
    if (role !== "user" && role !== "assistant") return [];
    const content = textFromAgUiMessage(message).trim();
    return content ? [{ role, content }] : [];
  });
}

function normalizeFrameType(type: unknown): CopilotStreamFrame["type"] | null {
  if (typeof type !== "string") return null;
  const mapped: Record<string, CopilotStreamFrame["type"]> = {
    text: "text",
    reasoning: "reasoning",
    toolCallStart: "toolCallStart",
    tool_call_start: "toolCallStart",
    toolCallResult: "toolCallResult",
    tool_call_result: "toolCallResult",
    responseBlock: "responseBlock",
    response_block: "responseBlock",
    source: "source",
    usage: "usage",
    done: "done",
    error: "error",
  };
  return mapped[type] ?? null;
}

function pick<T>(raw: Record<string, unknown>, camelKey: string, snakeKey: string): T {
  return (raw[camelKey] ?? raw[snakeKey]) as T;
}

export function normalizeCopilotFrame(payload: unknown): CopilotStreamFrame | null {
  if (!payload || typeof payload !== "object") return null;
  const raw = payload as Record<string, unknown>;
  const type = normalizeFrameType(raw.type);
  if (!type) return null;
  const base = {
    type,
    conversationId: pick<string>(raw, "conversationId", "conversation_id"),
    runId: pick<string>(raw, "runId", "run_id"),
    threadId: pick<string | undefined>(raw, "threadId", "thread_id"),
    assistantMessageId: pick<string | undefined>(raw, "assistantMessageId", "assistant_message_id"),
    parentMessageId: pick<string | null | undefined>(raw, "parentMessageId", "parent_message_id") ?? null,
    sequenceNumber: Number(pick<number | undefined>(raw, "sequenceNumber", "sequence_number") ?? -1),
  };
  if (!base.conversationId || !base.runId) return null;

  switch (type) {
    case "text":
      return { ...base, type, delta: pick<string>(raw, "delta", "delta") ?? "" };
    case "reasoning":
      return {
        ...base,
        type,
        reasoningMessageId: pick<string | undefined>(raw, "reasoningMessageId", "reasoning_message_id"),
        text: pick<string>(raw, "text", "text") ?? "",
      };
    case "toolCallStart":
      return {
        ...base,
        type,
        toolCallId: pick<string>(raw, "toolCallId", "tool_call_id"),
        toolName: pick<string>(raw, "toolName", "tool_name"),
        args: (pick(raw, "args", "args") ?? {}) as Record<string, unknown>,
      };
    case "toolCallResult":
      return {
        ...base,
        type,
        toolCallId: pick<string>(raw, "toolCallId", "tool_call_id"),
        toolResultMessageId: pick<string | undefined>(raw, "toolResultMessageId", "tool_result_message_id"),
        result: pick(raw, "result", "result"),
        isError: Boolean(pick(raw, "isError", "is_error")),
      };
    case "responseBlock":
      return {
        ...base,
        type,
        blockId: pick<string>(raw, "blockId", "block_id"),
        block: pick(raw, "block", "block"),
      } as Extract<CopilotStreamFrame, { type: "responseBlock" }>;
    case "source":
      return {
        ...base,
        type,
        sourceId: pick<string>(raw, "sourceId", "source_id"),
        title: pick<string>(raw, "title", "title") ?? "FinSight source",
      };
    case "usage":
      return {
        ...base,
        type,
        providerId: pick<string>(raw, "providerId", "provider_id") ?? "unknown",
        modelId: pick<string>(raw, "modelId", "model_id") ?? "unknown",
        elapsedMs: Number(pick(raw, "elapsedMs", "elapsed_ms") ?? 0),
        toolCount: Number(pick(raw, "toolCount", "tool_count") ?? 0),
      };
    case "done":
      return {
        ...base,
        type,
        messageId: pick<string>(raw, "messageId", "message_id"),
        bundleId: pick<string | null>(raw, "bundleId", "bundle_id") ?? null,
        toolTrace: (pick(raw, "toolTrace", "tool_trace") ?? []) as string[],
        followUpQuestions: (pick(raw, "followUpQuestions", "follow_up_questions") ?? []) as string[],
        actionLabel: pick<string | null>(raw, "actionLabel", "action_label") ?? null,
        actionPath: pick<string | null>(raw, "actionPath", "action_path") ?? null,
        providerId: pick<string>(raw, "providerId", "provider_id") ?? "unknown",
        modelId: pick<string>(raw, "modelId", "model_id") ?? "unknown",
        elapsedMs: Number(pick(raw, "elapsedMs", "elapsed_ms") ?? 0),
        toolCount: Number(pick(raw, "toolCount", "tool_count") ?? 0),
      };
    case "error":
      return {
        ...base,
        type,
        code: pick<string>(raw, "code", "code") ?? "copilot.error",
        message: pick<string>(raw, "message", "message") ?? "Copilot request failed.",
      };
  }
}

type AgentOptions = {
  conversationId?: string | null;
  onDone?: (payload: Extract<CopilotStreamFrame, { type: "done" }>) => void;
};

export class TauriAgUiAgent extends AbstractAgent {
  private conversationId: string | null;
  private onDone?: (payload: Extract<CopilotStreamFrame, { type: "done" }>) => void;

  constructor(options: AgentOptions = {}) {
    super({
      agentId: "finsight-tauri-ag-ui",
      description: "FinSight Copilot AG-UI runtime backed by Tauri stream events",
      threadId: options.conversationId ?? "finsight-copilot-ag-ui-thread",
    });
    this.conversationId = options.conversationId ?? null;
    this.onDone = options.onDone;
  }

  getConversationId() {
    return this.conversationId;
  }

  setConversationId(conversationId: string | null) {
    this.conversationId = conversationId;
  }

  override run(input: RunAgentInput): Observable<BaseEvent> {
    return new Observable<BaseEvent>((subscriber) => {
      const runId = input.runId || createId("agui-run");
      const assistantMessageId = createId("agui-assistant");
      const reasoningMessageId = createId("agui-reasoning");
      const toolArgsById = new Map<string, string>();
      const seenSequences = new Set<number>();
      let lastSequence = -1;
      let reasoningOpen = false;
      let textStarted = false;
      let completed = false;
      let disposed = false;
      let unlisten: (() => void) | null = null;
      let firstFrameWatchdog: ReturnType<typeof setTimeout> | null = null;

      const emit = (event: BaseEvent) => {
        if (!subscriber.closed && !disposed) subscriber.next(event);
      };

      const fail = (message: string, code = "copilot.agui") => {
        if (completed) return;
        completed = true;
        if (firstFrameWatchdog) clearTimeout(firstFrameWatchdog);
        unlisten?.();
        unlisten = null;
        if (reasoningOpen) {
          emit({ type: "REASONING_MESSAGE_END", messageId: reasoningMessageId } as BaseEvent);
          emit({ type: "REASONING_END", messageId: reasoningMessageId } as BaseEvent);
        }
        if (!textStarted) {
          emit({ type: "TEXT_MESSAGE_START", messageId: assistantMessageId } as BaseEvent);
          textStarted = true;
        }
        emit({ type: "TEXT_MESSAGE_CONTENT", messageId: assistantMessageId, delta: message } as BaseEvent);
        emit({ type: "TEXT_MESSAGE_END", messageId: assistantMessageId } as BaseEvent);
        emit({ type: "RUN_ERROR", message, code } as BaseEvent);
        subscriber.complete();
      };

      const handleFrame = (frame: CopilotStreamFrame) => {
        if (frame.runId !== runId || frame.conversationId !== this.conversationId) return;
        if (firstFrameWatchdog) {
          clearTimeout(firstFrameWatchdog);
          firstFrameWatchdog = null;
        }

        const sequenceNumber = (frame as { sequenceNumber?: number }).sequenceNumber;
        if (typeof sequenceNumber === "number" && sequenceNumber >= 0) {
          if (seenSequences.has(sequenceNumber)) return;
          if (sequenceNumber < lastSequence) return;
          if (sequenceNumber > lastSequence + 1 && lastSequence >= 0) {
            fail(`Received out-of-order Copilot stream frame ${sequenceNumber} after ${lastSequence}.`);
            return;
          }
          seenSequences.add(sequenceNumber);
          lastSequence = sequenceNumber;
        }

        switch (frame.type) {
          case "reasoning": {
            if (!reasoningOpen) {
              emit({ type: "REASONING_START", messageId: reasoningMessageId } as BaseEvent);
              emit({ type: "REASONING_MESSAGE_START", messageId: reasoningMessageId } as BaseEvent);
              reasoningOpen = true;
            }
            emit({
              type: "REASONING_MESSAGE_CONTENT",
              messageId: reasoningMessageId,
              delta: frame.text,
            } as BaseEvent);
            break;
          }
          case "toolCallStart": {
            const argsText = JSON.stringify(frame.args ?? {});
            toolArgsById.set(frame.toolCallId, argsText);
            emit({
              type: "TOOL_CALL_START",
              toolCallId: frame.toolCallId,
              toolCallName: frame.toolName,
              parentMessageId: assistantMessageId,
            } as BaseEvent);
            if (argsText !== "{}") {
              emit({ type: "TOOL_CALL_ARGS", toolCallId: frame.toolCallId, delta: argsText } as BaseEvent);
            }
            emit({ type: "TOOL_CALL_END", toolCallId: frame.toolCallId } as BaseEvent);
            break;
          }
          case "toolCallResult": {
            emit({
              type: "TOOL_CALL_RESULT",
              messageId: frame.toolResultMessageId ?? createId("tool-result"),
              toolCallId: frame.toolCallId,
              content: JSON.stringify(frame.result ?? { ok: !frame.isError }),
              role: "tool",
            } as BaseEvent);
            break;
          }
          case "responseBlock": {
            const artifactPayload = serializeFinanceArtifactEnvelope({
                artifactId: frame.blockId,
                schemaVersion: 1,
                kind: "artifact",
                component: "FinSightResponseBlock",
                props: { block: frame.block },
                sourceToolName: null,
                createdAt: new Date().toISOString(),
            });
            if (!artifactPayload) {
              emit({
                type: "CUSTOM",
                name: "finsight.invalid_artifact",
                value: { artifactId: frame.blockId, reason: "invalid_or_oversized" },
              } as BaseEvent);
              break;
            }
            const toolCallId = `artifact-${frame.blockId}`;
            emit({
              type: "TOOL_CALL_START",
              toolCallId,
              toolCallName: "render_finance_artifact",
              parentMessageId: assistantMessageId,
            } as BaseEvent);
            emit({ type: "TOOL_CALL_END", toolCallId } as BaseEvent);
            emit({
              type: "TOOL_CALL_RESULT",
              messageId: createId("artifact-result"),
              toolCallId,
              content: artifactPayload,
              role: "tool",
            } as BaseEvent);
            break;
          }
          case "source": {
            emit({
              type: "CUSTOM",
              name: "finsight.source",
              value: { id: frame.sourceId, title: frame.title },
            } as BaseEvent);
            break;
          }
          case "text": {
            if (!textStarted) {
              emit({ type: "TEXT_MESSAGE_START", messageId: assistantMessageId } as BaseEvent);
              textStarted = true;
            }
            emit({
              type: "TEXT_MESSAGE_CONTENT",
              messageId: assistantMessageId,
              delta: frame.delta,
            } as BaseEvent);
            break;
          }
          case "usage":
            emit({
              type: "CUSTOM",
              name: "finsight.usage",
              value: {
                providerId: frame.providerId,
                modelId: frame.modelId,
                elapsedMs: frame.elapsedMs,
                toolCount: frame.toolCount,
              },
            } as BaseEvent);
            break;
          case "error":
            fail(frame.message, frame.code);
            break;
          case "done":
            if (firstFrameWatchdog) clearTimeout(firstFrameWatchdog);
            this.onDone?.(frame);
            if (reasoningOpen) {
              emit({ type: "REASONING_MESSAGE_END", messageId: reasoningMessageId } as BaseEvent);
              emit({ type: "REASONING_END", messageId: reasoningMessageId } as BaseEvent);
            }
            if (frame.bundleId) {
              const approvalToolCallId = `approval-${frame.bundleId}`;
              emit({
                type: "TOOL_CALL_START",
                toolCallId: approvalToolCallId,
                toolCallName: "request_action_approval",
                parentMessageId: assistantMessageId,
              } as BaseEvent);
              emit({
                type: "TOOL_CALL_ARGS",
                toolCallId: approvalToolCallId,
                delta: JSON.stringify({ bundleId: frame.bundleId }),
              } as BaseEvent);
              emit({ type: "TOOL_CALL_END", toolCallId: approvalToolCallId } as BaseEvent);
              emit({
                type: "TOOL_CALL_RESULT",
                messageId: createId("approval-result"),
                toolCallId: approvalToolCallId,
                content: JSON.stringify({
                  kind: "approval_request",
                  bundleId: frame.bundleId,
                  source: "backend",
                  runId,
                }),
                role: "tool",
              } as BaseEvent);
            }
            if (textStarted) emit({ type: "TEXT_MESSAGE_END", messageId: assistantMessageId } as BaseEvent);
            else {
              emit({ type: "TEXT_MESSAGE_START", messageId: assistantMessageId } as BaseEvent);
              emit({ type: "TEXT_MESSAGE_CONTENT", messageId: assistantMessageId, delta: "" } as BaseEvent);
              emit({ type: "TEXT_MESSAGE_END", messageId: assistantMessageId } as BaseEvent);
            }
            completed = true;
            emit({ type: "RUN_FINISHED", runId, outcome: { type: "success" } } as BaseEvent);
            subscriber.complete();
            break;
        }
      };

      const start = async () => {
        try {
          if (!this.conversationId) {
            this.conversationId = unwrapCommandResult(await commands.createConversation());
          }

          const latestMessage = input.messages[input.messages.length - 1];
          const text = textFromAgUiMessage(latestMessage).trim();
          if (!text) {
            fail("Cannot send an empty Copilot message.", "copilot.empty_message");
            return;
          }

          unlisten = await listen<unknown>("copilot-stream-frame", (event) => {
            const frame = normalizeCopilotFrame(event.payload);
            if (frame) handleFrame(frame);
          });

          emit({ type: "RUN_STARTED", runId } as BaseEvent);
          firstFrameWatchdog = setTimeout(() => {
            fail(
              "Copilot did not emit a stream frame within 15 seconds. The run was stopped so the UI can recover.",
              "copilot.stream_timeout",
            );
          }, 15_000);

          const result = await commands.streamCopilotMessage(
            this.conversationId,
            runId,
            text,
            historyFromAgUiMessages(input.messages),
            null,
          );

          if (result.status === "error") fail(safeCommandErrorMessage(result.error), "copilot.command_error");
        } catch (error) {
          fail(error instanceof Error ? error.message : safeCommandErrorMessage(error), "copilot.exception");
        }
      };

      void start();

      return () => {
        disposed = true;
        if (firstFrameWatchdog) clearTimeout(firstFrameWatchdog);
        unlisten?.();
      };
    });
  }
}
