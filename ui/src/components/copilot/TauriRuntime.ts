import { createElement, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  RuntimeAdapterProvider,
  useAui,
  useLocalRuntime,
  useRemoteThreadListRuntime,
} from "@assistant-ui/react";
import type {
  AssistantRuntime,
  ChatModelAdapter,
  ChatModelRunOptions,
  ChatModelRunResult,
  ExportedMessageRepositoryItem,
  RemoteThreadListAdapter,
  ThreadHistoryAdapter,
  ThreadAssistantMessagePart,
  ThreadMessage,
  ThreadMessageLike,
  ToolCallMessagePart,
} from "@assistant-ui/react";
import { useQueryClient } from "@tanstack/react-query";
import { commands } from "../../api/client";
import type {
  ChatHistoryEntry,
  ConversationMessage,
  CopilotStreamFrame,
  CopilotDonePayload,
  CopilotResponseBlock,
} from "../../api/client";

export interface MessageMeta {
  bundleId?: string;
  toolTrace?: string[];
  plan?: string[];
  followUpQuestions?: string[];
  actionLabel?: string;
  actionPath?: string;
  providerId?: string;
  modelId?: string;
  elapsedMs?: number;
  toolCount?: number;
}

type MetaByMessageId = Record<string, MessageMeta>;

function unwrapCommandResult<T>(result: { status: "ok"; data: T } | { status: "error"; error: unknown }): T {
  if (result.status === "error") throw result.error;
  return result.data;
}

type StreamEvent =
  | { type: "frame"; frame: CopilotStreamFrame }
  | { type: "error"; error: unknown };

type EventQueue<T> = {
  push(value: T): void;
  end(): void;
  fail(error: unknown): void;
  next(signal: AbortSignal): Promise<T | null>;
};

function createEventQueue<T>(): EventQueue<T> {
  const values: T[] = [];
  const waiters: Array<{
    resolve: (value: T | null) => void;
    reject: (reason?: unknown) => void;
    cleanup: () => void;
  }> = [];
  let ended = false;
  let failed: unknown;

  const settleNext = () => {
    while (waiters.length > 0 && values.length > 0) {
      const waiter = waiters.shift();
      const value = values.shift();
      if (!waiter || value === undefined) continue;
      waiter.cleanup();
      waiter.resolve(value);
    }

    if (failed !== undefined) {
      while (waiters.length > 0) {
        const waiter = waiters.shift();
        waiter?.cleanup();
        waiter?.reject(failed);
      }
      return;
    }

    if (ended) {
      while (waiters.length > 0) {
        const waiter = waiters.shift();
        waiter?.cleanup();
        waiter?.resolve(null);
      }
    }
  };

  return {
    push(value) {
      if (ended || failed !== undefined) return;
      values.push(value);
      settleNext();
    },
    end() {
      ended = true;
      settleNext();
    },
    fail(error) {
      failed = error;
      settleNext();
    },
    next(signal) {
      if (values.length > 0) return Promise.resolve(values.shift() ?? null);
      if (failed !== undefined) return Promise.reject(failed);
      if (ended) return Promise.resolve(null);
      if (signal.aborted) return Promise.resolve(null);

      return new Promise<T | null>((resolve, reject) => {
        const onAbort = () => {
          cleanup();
          const index = waiters.findIndex((waiter) => waiter.resolve === resolve);
          if (index >= 0) waiters.splice(index, 1);
          resolve(null);
        };
        const cleanup = () => signal.removeEventListener("abort", onAbort);
        signal.addEventListener("abort", onAbort, { once: true });
        waiters.push({ resolve, reject, cleanup });
      });
    },
  };
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

function pickFrameValue<T = unknown>(raw: Record<string, unknown>, camelKey: string, snakeKey: string): T {
  return (raw[camelKey] ?? raw[snakeKey]) as T;
}

function normalizeCopilotStreamFrame(payload: unknown): CopilotStreamFrame | null {
  if (!payload || typeof payload !== "object") return null;
  const raw = payload as Record<string, unknown>;
  const type = normalizeFrameType(raw.type);
  if (!type) return null;

  const base = {
    type,
    conversationId: pickFrameValue<string>(raw, "conversationId", "conversation_id"),
    runId: pickFrameValue<string>(raw, "runId", "run_id"),
    threadId: pickFrameValue<string | undefined>(raw, "threadId", "thread_id") ?? "",
    assistantMessageId: pickFrameValue<string | undefined>(raw, "assistantMessageId", "assistant_message_id") ?? "",
    parentMessageId: pickFrameValue<string | null | undefined>(raw, "parentMessageId", "parent_message_id") ?? null,
    sequenceNumber: Number(pickFrameValue(raw, "sequenceNumber", "sequence_number") ?? -1),
  };
  if (!base.conversationId || !base.runId) return null;

  switch (type) {
    case "text":
      return { ...base, type, delta: pickFrameValue<string>(raw, "delta", "delta") ?? "" };
    case "reasoning":
      return {
        ...base,
        type,
        reasoningMessageId: pickFrameValue<string | undefined>(raw, "reasoningMessageId", "reasoning_message_id") ?? "",
        text: pickFrameValue<string>(raw, "text", "text") ?? "",
      };
    case "toolCallStart":
      return {
        ...base,
        type,
        toolCallId: pickFrameValue<string>(raw, "toolCallId", "tool_call_id"),
        parentMessageId: pickFrameValue<string | null | undefined>(raw, "parentMessageId", "parent_message_id") ?? null,
        toolName: pickFrameValue<string>(raw, "toolName", "tool_name"),
        args: (pickFrameValue(raw, "args", "args") ?? {}) as Record<string, unknown>,
      };
    case "toolCallResult":
      return {
        ...base,
        type,
        toolCallId: pickFrameValue<string>(raw, "toolCallId", "tool_call_id"),
        toolResultMessageId: pickFrameValue<string | undefined>(raw, "toolResultMessageId", "tool_result_message_id") ?? "",
        result: pickFrameValue(raw, "result", "result"),
        isError: Boolean(pickFrameValue(raw, "isError", "is_error")),
      };
    case "responseBlock":
      return {
        ...base,
        type,
        blockId: pickFrameValue<string>(raw, "blockId", "block_id"),
        block: pickFrameValue(raw, "block", "block") as CopilotResponseBlock,
      };
    case "source":
      return {
        ...base,
        type,
        sourceId: pickFrameValue<string>(raw, "sourceId", "source_id"),
        title: pickFrameValue<string>(raw, "title", "title") ?? "FinSight source",
      };
    case "usage":
      return {
        ...base,
        type,
        providerId: pickFrameValue<string>(raw, "providerId", "provider_id") ?? "unknown",
        modelId: pickFrameValue<string>(raw, "modelId", "model_id") ?? "unknown",
        elapsedMs: Number(pickFrameValue(raw, "elapsedMs", "elapsed_ms") ?? 0),
        toolCount: Number(pickFrameValue(raw, "toolCount", "tool_count") ?? 0),
      };
    case "done":
      return {
        ...base,
        type,
        messageId: pickFrameValue<string>(raw, "messageId", "message_id"),
        bundleId: pickFrameValue<string | null>(raw, "bundleId", "bundle_id") ?? null,
        toolTrace: (pickFrameValue(raw, "toolTrace", "tool_trace") ?? []) as string[],
        followUpQuestions: (pickFrameValue(raw, "followUpQuestions", "follow_up_questions") ?? []) as string[],
        actionLabel: pickFrameValue<string | null>(raw, "actionLabel", "action_label") ?? null,
        actionPath: pickFrameValue<string | null>(raw, "actionPath", "action_path") ?? null,
        providerId: pickFrameValue<string>(raw, "providerId", "provider_id") ?? "unknown",
        modelId: pickFrameValue<string>(raw, "modelId", "model_id") ?? "unknown",
        elapsedMs: Number(pickFrameValue(raw, "elapsedMs", "elapsed_ms") ?? 0),
        toolCount: Number(pickFrameValue(raw, "toolCount", "tool_count") ?? 0),
      };
    case "error":
      return {
        ...base,
        type,
        code: pickFrameValue<string>(raw, "code", "code") ?? "copilot.error",
        message: pickFrameValue<string>(raw, "message", "message") ?? "Copilot request failed.",
      };
    case "plan":
      // The legacy (non-AG-UI) runtime never surfaces the Plan section — it
      // reads MessageMeta.plan back from persisted agUiMetadataJson on reload
      // instead (see TauriRuntime's history-load path), so live Plan frames
      // are intentionally dropped here rather than converted to an event.
      return null;
  }
}

export function formatCommandError(error: unknown) {
  if (error && typeof error === "object") {
    const maybeAppError = error as { code?: unknown; message?: unknown };
    if (typeof maybeAppError.message === "string") {
      return typeof maybeAppError.code === "string"
        ? `${maybeAppError.code}: ${maybeAppError.message}`
        : maybeAppError.message;
    }
  }
  return error instanceof Error ? error.message : "An error occurred";
}

function formatUserFacingError(error: unknown) {
  const errorText = formatCommandError(error);
  if (errorText.includes("no_provider") || errorText.includes("Configure an AI provider")) {
    return "AI provider not configured. Go to Settings -> Agent to set up your AI provider.";
  }
  if (errorText.includes("agent.empty_response")) {
    return "Copilot finished without a text response. Check the configured AI provider/model in Settings -> Agent, then try again.";
  }
  return `Copilot request failed: ${errorText}`;
}

function textFromMessage(message: ThreadMessage | ThreadMessageLike) {
  const content = typeof message.content === "string" ? [] : message.content;
  return content
    .filter((part): part is { type: "text"; text: string } => part.type === "text")
    .map((part) => part.text)
    .join("\n");
}

function historyFromMessages(messages: readonly ThreadMessage[]): ChatHistoryEntry[] {
  return messages.slice(0, -1).flatMap((message) => {
    if (message.role !== "user" && message.role !== "assistant") return [];
    const content = textFromMessage(message).trim();
    if (!content) return [];
    return [{ role: message.role, content }];
  });
}

function backendMessageId(message: ThreadMessage | ThreadMessageLike | undefined) {
  const custom = message?.metadata?.custom as { messageId?: unknown } | undefined;
  return typeof custom?.messageId === "string" ? custom.messageId : undefined;
}

function metaFromDone(payload: Extract<CopilotStreamFrame, { type: "done" }>): MessageMeta {
  return {
    bundleId: payload.bundleId ?? undefined,
    toolTrace: payload.toolTrace,
    followUpQuestions: payload.followUpQuestions,
    actionLabel: payload.actionLabel ?? undefined,
    actionPath: payload.actionPath ?? undefined,
    providerId: payload.providerId,
    modelId: payload.modelId,
    elapsedMs: payload.elapsedMs,
    toolCount: payload.toolCount,
  };
}

function createRunId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `run-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function conversationMessagesToThreadMessages(
  messages: ConversationMessage[]
): ThreadMessageLike[] {
  return messages.map((message) => ({
    id: message.id,
    role: message.role as "user" | "assistant",
    content: parseStoredParts(message),
    status:
      message.role === "assistant"
        ? ({ type: "complete", reason: "stop" } as const)
        : undefined,
    createdAt: new Date(message.createdAt),
    metadata: {
      custom: {
        messageId: message.id,
        ...(message.actionBundleId ? { bundleId: message.actionBundleId } : {}),
        ...(message.toolTrace ? { toolTrace: message.toolTrace } : {}),
      },
    },
  }));
}

function parseStoredParts(message: ConversationMessage): ThreadMessageLike["content"] {
  const raw = (message as ConversationMessage & { partsJson?: string | null }).partsJson;
  if (typeof raw === "string" && raw.trim()) {
    try {
      const parsed = JSON.parse(raw) as unknown;
      if (Array.isArray(parsed) && parsed.every(isMessagePartLike)) {
        return parsed as unknown as ThreadMessageLike["content"];
      }
    } catch {
      // Fall through to text fallback.
    }
  }
  return [{ type: "text", text: message.content }];
}

function isMessagePartLike(value: unknown): value is { type: string } {
  return Boolean(value && typeof value === "object" && typeof (value as { type?: unknown }).type === "string");
}

export function createTauriChatModelAdapter({
  ensureConversationId,
  onDone,
}: {
  ensureConversationId: () => Promise<string>;
  onDone?: (payload: Extract<CopilotStreamFrame, { type: "done" }>, meta: MessageMeta) => void;
}): ChatModelAdapter {
  return {
    async *run(options: ChatModelRunOptions): AsyncGenerator<ChatModelRunResult, void> {
      const conversationId = await ensureConversationId();

      const latestMessage = options.messages[options.messages.length - 1];
      const text = latestMessage ? textFromMessage(latestMessage).trim() : "";
      if (!text) return;
      const sourceMessageId = latestMessage?.role === "user" ? backendMessageId(latestMessage) : undefined;

      const runId = createRunId();
      const queue = createEventQueue<StreamEvent>();
      const cleanup: Array<() => void> = [];
      let bufferedText = "";
      let reasoningText = "";
      let usage: Extract<CopilotStreamFrame, { type: "usage" }> | null = null;
      const content: ThreadAssistantMessagePart[] = [];
      const toolIndexById = new Map<string, number>();

      const yieldContent = (): ChatModelRunResult => ({
        content: [...content],
        metadata: usage
          ? {
              timing: {
                streamStartTime: Date.now() - usage.elapsedMs,
                totalStreamTime: usage.elapsedMs,
                totalChunks: content.length,
                toolCallCount: usage.toolCount,
              },
              custom: {
                providerId: usage.providerId,
                modelId: usage.modelId,
                elapsedMs: usage.elapsedMs,
                toolCount: usage.toolCount,
              },
            }
          : undefined,
      });

      try {
        cleanup.push(
          await listen<unknown>("copilot-stream-frame", (event) => {
            const frame = normalizeCopilotStreamFrame(event.payload);
            if (!frame) return;
            if (
              frame.conversationId !== conversationId ||
              frame.runId !== runId
            ) {
              return;
            }
            queue.push({ type: "frame", frame });
            if (frame.type === "done" || frame.type === "error") queue.end();
          })
        );

        void commands
          .streamCopilotMessage(
            conversationId,
            runId,
            text,
            historyFromMessages(options.messages),
            sourceMessageId ?? null
          )
          .then((result) => {
            if (result.status === "error") queue.fail(result.error);
          })
          .catch((error: unknown) => queue.fail(error));

        while (!options.abortSignal.aborted) {
          const event = await queue.next(options.abortSignal);
          if (!event) break;

          if (event.type !== "frame") {
            queue.fail(event.error);
            continue;
          }

          const frame = event.frame;
          if (frame.type === "reasoning") {
            reasoningText += frame.text;
            const existingIndex = content.findIndex((part) => part.type === "reasoning");
            const part = { type: "reasoning", text: reasoningText } as const;
            if (existingIndex >= 0) content[existingIndex] = part;
            else content.push(part);
            yield yieldContent();
            continue;
          }

          if (frame.type === "toolCallStart") {
            const part: ToolCallMessagePart = {
              type: "tool-call",
              toolCallId: frame.toolCallId,
              toolName: frame.toolName,
              args: frame.args as ToolCallMessagePart["args"],
              argsText: JSON.stringify(frame.args ?? {}),
            };
            toolIndexById.set(frame.toolCallId, content.length);
            content.push(part);
            yield yieldContent();
            continue;
          }

          if (frame.type === "toolCallResult") {
            const index = toolIndexById.get(frame.toolCallId);
            if (index !== undefined) {
              const current = content[index];
              if (current?.type === "tool-call") {
                content[index] = {
                  ...current,
                  result: frame.result,
                  isError: frame.isError,
                } as ToolCallMessagePart;
              }
            }
            yield yieldContent();
            continue;
          }

          if (frame.type === "responseBlock") {
            content.push({
              type: "generative-ui",
              id: frame.blockId,
              spec: {
                root: {
                  component: "FinSightResponseBlock",
                  props: { block: frame.block },
                },
              },
            } as ThreadAssistantMessagePart);
            yield yieldContent();
            continue;
          }

          if (frame.type === "source") {
            content.push({
              type: "source",
              sourceType: "document",
              id: frame.sourceId,
              title: frame.title,
              mediaType: "application/x-finsight-source",
            });
            yield yieldContent();
            continue;
          }

          if (frame.type === "text") {
            bufferedText += frame.delta;
            const existingIndex = content.findIndex((part) => part.type === "text");
            const part = { type: "text", text: bufferedText } as const;
            if (existingIndex >= 0) content[existingIndex] = part;
            else content.push(part);
            yield yieldContent();
            continue;
          }

          if (frame.type === "usage") {
            usage = frame;
            yield yieldContent();
            continue;
          }

          if (frame.type === "error") {
            throw new Error(`${frame.code}: ${frame.message}`);
          }

          if (frame.type === "done") {
            const meta = metaFromDone(frame);
            onDone?.(frame, meta);
            yield {
              content: [...content],
              status: { type: "complete", reason: "stop" },
              metadata: {
                timing: {
                  streamStartTime: Date.now() - frame.elapsedMs,
                  totalStreamTime: frame.elapsedMs,
                  totalChunks: content.length,
                  toolCallCount: frame.toolCount,
                },
                custom: {
                  messageId: frame.messageId,
                  bundleId: meta.bundleId,
                  providerId: frame.providerId,
                  modelId: frame.modelId,
                  elapsedMs: frame.elapsedMs,
                  toolCount: frame.toolCount,
                },
              },
            };
            break;
          }
        }
      } catch (error) {
        yield {
          content: [{ type: "text", text: formatUserFacingError(error) }],
          status: { type: "incomplete", reason: "error", error: formatCommandError(error) },
        };
      } finally {
        cleanup.forEach((unlisten) => unlisten());
      }
    },
  };
}

export function buildMetaFromMessages(messages: ConversationMessage[]): MetaByMessageId {
  return Object.fromEntries(
    messages.flatMap((message) => {
      const meta: MessageMeta = {};
      if (message.actionBundleId) meta.bundleId = message.actionBundleId;
      const agUiMetadataJson = (message as ConversationMessage & { agUiMetadataJson?: string | null }).agUiMetadataJson;
      if (agUiMetadataJson) {
        try {
          const parsed = JSON.parse(agUiMetadataJson) as Record<string, unknown>;
          if (typeof parsed.bundleId === "string") meta.bundleId = parsed.bundleId;
          if (Array.isArray(parsed.toolTrace)) {
            meta.toolTrace = parsed.toolTrace.filter((item): item is string => typeof item === "string");
          }
          if (Array.isArray(parsed.plan)) {
            meta.plan = parsed.plan.filter((item): item is string => typeof item === "string");
          }
          if (Array.isArray(parsed.followUpQuestions)) {
            meta.followUpQuestions = parsed.followUpQuestions.filter((item): item is string => typeof item === "string");
          }
          if (typeof parsed.actionLabel === "string") meta.actionLabel = parsed.actionLabel;
          if (typeof parsed.actionPath === "string") meta.actionPath = parsed.actionPath;
          if (typeof parsed.providerId === "string") meta.providerId = parsed.providerId;
          if (typeof parsed.modelId === "string") meta.modelId = parsed.modelId;
          if (typeof parsed.elapsedMs === "number") meta.elapsedMs = parsed.elapsedMs;
          if (typeof parsed.toolCount === "number") meta.toolCount = parsed.toolCount;
        } catch {
          // Ignore corrupt metadata and fall back to legacy fields.
        }
      }
      if (message.toolTrace) {
        try {
          const parsed = JSON.parse(message.toolTrace) as unknown;
          meta.toolTrace = Array.isArray(parsed)
            ? parsed.filter((item): item is string => typeof item === "string")
            : [message.toolTrace];
        } catch {
          meta.toolTrace = [message.toolTrace];
        }
      }
      return Object.keys(meta).length > 0 ? [[message.id, meta]] : [];
    })
  );
}

function FinSightThreadHistoryProvider({
  children,
  onLoadedMeta,
}: {
  children: ReactNode;
  onLoadedMeta: (meta: MetaByMessageId) => void;
}) {
  const aui = useAui();
  const history = useMemo<ThreadHistoryAdapter>(
    () => ({
      async load() {
        const { remoteId } = aui.threadListItem().getState();
        if (!remoteId) return { messages: [] };

        const messages = unwrapCommandResult(
          await commands.getConversationMessages(remoteId)
        );
        onLoadedMeta(buildMetaFromMessages(messages));
        return {
          messages: conversationMessagesToThreadMessages(messages).map((message) => ({
            parentId: null,
            message: message as ThreadMessage,
          })),
        };
      },
      async append(_item: ExportedMessageRepositoryItem) {
        // Rust streamCopilotMessage persists user and assistant turns atomically.
      },
      async update(item: ExportedMessageRepositoryItem) {
        if (item.message.role !== "user") return;
        const messageId = backendMessageId(item.message);
        const conversationId = aui.threadListItem().getState().remoteId;
        const content = textFromMessage(item.message).trim();
        if (!messageId || !conversationId || !content) return;
        unwrapCommandResult(
          await commands.editConversationUserMessage({
            conversationId,
            messageId,
            content,
          })
        );
      },
      async delete(items: ExportedMessageRepositoryItem[]) {
        const conversationId = aui.threadListItem().getState().remoteId;
        const firstBackendId = items
          .map((item) => backendMessageId(item.message))
          .find((id): id is string => Boolean(id));
        if (!conversationId || !firstBackendId) return;
        unwrapCommandResult(
          await commands.deleteConversationMessagesAfter(conversationId, firstBackendId)
        );
      },
    }),
    [aui, onLoadedMeta]
  );

  return createElement(RuntimeAdapterProvider, { adapters: { history }, children });
}

export function useTauriCopilotRuntime(initialConversationId?: string | null): {
  runtime: AssistantRuntime;
  latestMeta: MessageMeta | null;
  metaByMessageId: MetaByMessageId;
} {
  const [latestMeta, setLatestMeta] = useState<MessageMeta | null>(null);
  const [metaByMessageId, setMetaByMessageId] = useState<MetaByMessageId>({});
  const queryClient = useQueryClient();
  const convIdRef = useRef<string | null>(initialConversationId ?? null);

  const onDone = useCallback(
    (payload: CopilotDonePayload, meta: MessageMeta) => {
      setLatestMeta(meta);
      setMetaByMessageId((prev) => ({ ...prev, [payload.messageId]: meta }));
      void queryClient.invalidateQueries({ queryKey: ["conversations"] });
      void queryClient.invalidateQueries({
        queryKey: ["conversation-messages", payload.conversationId],
      });
      void queryClient.invalidateQueries({ queryKey: ["action-bundles"] });
    },
    [queryClient]
  );

  const ensureConversationId = useCallback(async () => {
    if (convIdRef.current) return convIdRef.current;
    const conversationId = unwrapCommandResult(await commands.createConversation());
    convIdRef.current = conversationId;
    return conversationId;
  }, []);

  const adapter = useMemo(
    () =>
      createTauriChatModelAdapter({
        ensureConversationId,
        onDone,
      }),
    [ensureConversationId, onDone]
  );

  const setLoadedMeta = useCallback((meta: MetaByMessageId) => {
    setMetaByMessageId(meta);
  }, []);

  const threadListAdapter = useMemo<RemoteThreadListAdapter>(
    () => ({
      async list() {
        const conversations = unwrapCommandResult(await commands.listConversations());
        return {
          threads: conversations.map((conversation) => ({
            status: "regular" as const,
            remoteId: conversation.id,
            title: conversation.title,
            lastMessageAt: new Date(conversation.updatedAt),
            custom: {
              createdAt: conversation.createdAt,
              messageCount: conversation.messageCount,
            },
          })),
        };
      },
      async initialize() {
        const remoteId = unwrapCommandResult(await commands.createConversation());
        convIdRef.current = remoteId;
        void queryClient.invalidateQueries({ queryKey: ["conversations"] });
        return { remoteId, externalId: undefined };
      },
      async rename(_remoteId, _newTitle) {
        // Conversation titles are generated by the Rust agent today.
      },
      async archive(_remoteId) {
        // FinSight does not yet persist archived Copilot threads.
      },
      async unarchive(_remoteId) {
        // FinSight does not yet persist archived Copilot threads.
      },
      async delete(remoteId) {
        unwrapCommandResult(await commands.deleteConversation(remoteId));
        if (convIdRef.current === remoteId) convIdRef.current = null;
        void queryClient.invalidateQueries({ queryKey: ["conversations"] });
      },
      async fetch(remoteId) {
        const conversations = unwrapCommandResult(await commands.listConversations());
        const conversation = conversations.find((item) => item.id === remoteId);
        if (!conversation) throw new Error("Conversation not found.");
        return {
          status: "regular" as const,
          remoteId: conversation.id,
          title: conversation.title,
          lastMessageAt: new Date(conversation.updatedAt),
          custom: {
            createdAt: conversation.createdAt,
            messageCount: conversation.messageCount,
          },
        };
      },
      async generateTitle(_remoteId, messages) {
        return new ReadableStream<string>({
          start(controller) {
            const firstUserText =
              messages.find((message) => message.role === "user")
                ? textFromMessage(messages.find((message) => message.role === "user")!)
                : "New conversation";
            controller.enqueue(firstUserText.trim().slice(0, 48) || "New conversation");
            controller.close();
          },
        }) as never;
      },
      unstable_Provider({ children }) {
        return createElement(FinSightThreadHistoryProvider, { onLoadedMeta: setLoadedMeta, children });
      },
    }),
    [queryClient, setLoadedMeta]
  );

  const runtime = useRemoteThreadListRuntime({
    runtimeHook: () =>
      useLocalRuntime(adapter, {
        unstable_enableMessageQueue: true,
      }),
    adapter: threadListAdapter,
    threadId: initialConversationId ?? undefined,
    onThreadIdChange: (threadId) => {
      convIdRef.current = threadId ?? null;
      setLatestMeta(null);
    },
  });

  return {
    runtime,
    latestMeta,
    metaByMessageId,
  };
}
