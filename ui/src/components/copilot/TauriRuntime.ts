import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useLocalRuntime } from "@assistant-ui/react";
import type {
  AssistantRuntime,
  ChatModelAdapter,
  ChatModelRunOptions,
  ChatModelRunResult,
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
} from "../../api/client";

export interface MessageMeta {
  bundleId?: string;
  toolTrace?: string[];
  followUpQuestions?: string[];
  actionLabel?: string;
  actionPath?: string;
  providerId?: string;
  modelId?: string;
  elapsedMs?: number;
  toolCount?: number;
}

type MetaByMessageId = Record<string, MessageMeta>;

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
  getConversationId,
  onDone,
}: {
  getConversationId: () => string | null;
  onDone?: (payload: Extract<CopilotStreamFrame, { type: "done" }>, meta: MessageMeta) => void;
}): ChatModelAdapter {
  return {
    async *run(options: ChatModelRunOptions): AsyncGenerator<ChatModelRunResult, void> {
      const conversationId = getConversationId();
      if (!conversationId) {
        throw new Error("Start a conversation before sending a message.");
      }

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
          await listen<CopilotStreamFrame>("copilot-stream-frame", (event) => {
            if (
              event.payload.conversationId !== conversationId ||
              event.payload.runId !== runId
            ) {
              return;
            }
            queue.push({ type: "frame", frame: event.payload });
            if (event.payload.type === "done" || event.payload.type === "error") queue.end();
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

export function useTauriCopilotRuntime(conversationId: string | null): {
  runtime: AssistantRuntime;
  messages: ThreadMessageLike[];
  isRunning: boolean;
  latestMeta: MessageMeta | null;
  metaByMessageId: MetaByMessageId;
} {
  const [initialMessages, setInitialMessages] = useState<ThreadMessageLike[]>([]);
  const [latestMeta, setLatestMeta] = useState<MessageMeta | null>(null);
  const [metaByMessageId, setMetaByMessageId] = useState<MetaByMessageId>({});
  const queryClient = useQueryClient();
  const convIdRef = useRef<string | null>(conversationId);
  convIdRef.current = conversationId;

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

  const adapter = useMemo(
    () =>
      createTauriChatModelAdapter({
        getConversationId: () => convIdRef.current,
        onDone,
      }),
    [onDone]
  );

  const runtime = useLocalRuntime(adapter, { initialMessages });

  useEffect(() => {
    setLatestMeta(null);
    setMetaByMessageId({});
    if (!conversationId) {
      setInitialMessages([]);
      return;
    }

    let cancelled = false;
    commands
      .getConversationMessages(conversationId)
      .then((result) => {
        if (cancelled || result.status === "error") return;
        const loaded = conversationMessagesToThreadMessages(result.data);
        const nextMetaByMessageId = Object.fromEntries(
          result.data
            .filter((message) => message.actionBundleId)
            .map((message) => [
              message.id,
              { bundleId: message.actionBundleId ?? undefined } satisfies MessageMeta,
            ])
        );
        setInitialMessages(loaded);
        setMetaByMessageId(nextMetaByMessageId);
      })
      .catch(() => {
        if (!cancelled) setInitialMessages([]);
      });

    return () => {
      cancelled = true;
    };
  }, [conversationId]);

  return {
    runtime,
    messages: initialMessages,
    isRunning: false,
    latestMeta,
    metaByMessageId,
  };
}
