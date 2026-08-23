/**
 * The ONE parser for `copilot-stream-frame` event payloads, shared by both
 * chat runtimes (legacy TauriRuntime and the AG-UI runtime).
 *
 * Before this module the same ~130-line normalizer lived twice — once per
 * runtime — and had already drifted (optional-id defaults, plan handling).
 * Every new frame type must now be handled in exactly one place.
 *
 * The two runtimes still differ in three deliberate, documented ways:
 *  1. `plan` frames: the AG-UI runtime surfaces them live; the legacy runtime
 *     drops them (it reads the Plan section back from persisted
 *     agUiMetadataJson on reload instead).
 *  2. Optional string ids (`threadId`, `assistantMessageId`,
 *     `reasoningMessageId`, `toolResultMessageId`): legacy coerces missing →
 *     `""`; AG-UI passes `undefined` through.
 *  3. Nothing else. Any future difference must be added here as an explicit
 *     `variant` branch, not forked into a second parser.
 */
import type { CopilotResponseBlock, CopilotStreamFrame } from "../../api/client";
import type { MissingDataItem } from "../../api/client";

export type StreamFrameVariant = "legacy" | "agui";

function normalizeFrameType(
  type: unknown,
  variant: StreamFrameVariant,
): CopilotStreamFrame["type"] | null {
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
    // The legacy runtime never surfaces the Plan section live — it reads
    // MessageMeta.plan back from persisted agUiMetadataJson on reload instead
    // (see TauriRuntime's history-load path), so live Plan frames are
    // intentionally dropped there rather than converted to an event.
    ...(variant === "agui" ? { plan: "plan" } : {}),
    usage: "usage",
    done: "done",
    error: "error",
  };
  return mapped[type] ?? null;
}

function pick<T>(raw: Record<string, unknown>, camelKey: string, snakeKey: string): T {
  return (raw[camelKey] ?? raw[snakeKey]) as T;
}

function optId(
  raw: Record<string, unknown>,
  camelKey: string,
  snakeKey: string,
  variant: StreamFrameVariant,
): string | undefined {
  const v = pick<string | undefined>(raw, camelKey, snakeKey);
  return variant === "legacy" ? v ?? "" : v;
}

export function normalizeCopilotStreamFrame(
  payload: unknown,
  variant: StreamFrameVariant,
): CopilotStreamFrame | null {
  if (!payload || typeof payload !== "object") return null;
  const raw = payload as Record<string, unknown>;
  const type = normalizeFrameType(raw.type, variant);
  if (!type) return null;

  const base = {
    type,
    conversationId: pick<string>(raw, "conversationId", "conversation_id"),
    runId: pick<string>(raw, "runId", "run_id"),
    threadId: optId(raw, "threadId", "thread_id", variant),
    assistantMessageId: optId(raw, "assistantMessageId", "assistant_message_id", variant),
    parentMessageId: pick<string | null | undefined>(raw, "parentMessageId", "parent_message_id") ?? null,
    sequenceNumber: Number(pick(raw, "sequenceNumber", "sequence_number") ?? -1),
  };
  if (!base.conversationId || !base.runId) return null;

  switch (type) {
    case "text":
      return { ...base, type, delta: pick<string>(raw, "delta", "delta") ?? "" };
    case "reasoning":
      return {
        ...base,
        type,
        reasoningMessageId: optId(raw, "reasoningMessageId", "reasoning_message_id", variant),
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
        toolResultMessageId: optId(raw, "toolResultMessageId", "tool_result_message_id", variant),
        result: pick(raw, "result", "result"),
        isError: Boolean(pick(raw, "isError", "is_error")),
      };
    case "responseBlock":
      return {
        ...base,
        type,
        blockId: pick<string>(raw, "blockId", "block_id"),
        block: pick(raw, "block", "block") as CopilotResponseBlock,
      };
    case "source":
      return {
        ...base,
        type,
        sourceId: pick<string>(raw, "sourceId", "source_id"),
        title: pick<string>(raw, "title", "title") ?? "FinSight source",
      };
    case "plan":
      return {
        ...base,
        type,
        steps: (pick(raw, "steps", "steps") ?? []) as string[],
      };
    case "usage":
      return {
        ...base,
        type,
        providerId: pick<string>(raw, "providerId", "provider_id") ?? "unknown",
        modelId: pick<string>(raw, "modelId", "model_id") ?? "unknown",
        elapsedMs: Number(pick(raw, "elapsedMs", "elapsed_ms") ?? 0),
        toolCount: Number(pick(raw, "toolCount", "tool_count") ?? 0),
        cachedTokens: Number(pick(raw, "cachedTokens", "cached_tokens") ?? 0),
        promptTokens: Number(pick(raw, "promptTokens", "prompt_tokens") ?? 0),
      };
    case "done":
      return {
        ...base,
        type,
        messageId: pick<string>(raw, "messageId", "message_id"),
        bundleId: pick<string | null>(raw, "bundleId", "bundle_id") ?? null,
        toolTrace: (pick(raw, "toolTrace", "tool_trace") ?? []) as string[],
        followUpQuestions: (pick(raw, "followUpQuestions", "follow_up_questions") ?? []) as string[],
        missingData: (pick(raw, "missingData", "missing_data") ?? []) as MissingDataItem[],
        actionLabel: pick<string | null>(raw, "actionLabel", "action_label") ?? null,
        actionPath: pick<string | null>(raw, "actionPath", "action_path") ?? null,
        providerId: pick<string>(raw, "providerId", "provider_id") ?? "unknown",
        modelId: pick<string>(raw, "modelId", "model_id") ?? "unknown",
        elapsedMs: Number(pick(raw, "elapsedMs", "elapsed_ms") ?? 0),
        toolCount: Number(pick(raw, "toolCount", "tool_count") ?? 0),
        cachedTokens: Number(pick(raw, "cachedTokens", "cached_tokens") ?? 0),
        promptTokens: Number(pick(raw, "promptTokens", "prompt_tokens") ?? 0),
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
