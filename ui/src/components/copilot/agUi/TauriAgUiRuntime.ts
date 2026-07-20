import { useCallback, useMemo, useState } from "react";
import { fromAgUiMessages, useAgUiRuntime } from "@assistant-ui/react-ag-ui";
import type {
  AssistantRuntime,
  ExportedMessageRepositoryItem,
  ThreadHistoryAdapter,
  ThreadMessage,
  ThreadMessageLike,
} from "@assistant-ui/react";
import { useQueryClient } from "@tanstack/react-query";
import { commands } from "../../../api/client";
import type { ConversationMessage, CopilotStreamFrame } from "../../../api/client";
import { TauriAgUiAgent } from "./TauriAgUiAgent";
import {
  buildMetaFromMessages,
  conversationMessagesToThreadMessages,
  type MessageMeta,
} from "../TauriRuntime";

type MetaByMessageId = Record<string, MessageMeta>;

function unwrapCommandResult<T>(result: { status: "ok"; data: T } | { status: "error"; error: unknown }): T {
  if (result.status === "error") throw result.error;
  return result.data;
}

function textFromMessage(message: ThreadMessage | ThreadMessageLike) {
  const content = typeof message.content === "string" ? [] : message.content;
  return content
    .filter((part): part is { type: "text"; text: string } => part.type === "text")
    .map((part) => part.text)
    .join("\n");
}

function backendMessageId(message: ThreadMessage | ThreadMessageLike | undefined) {
  const custom = message?.metadata?.custom as { messageId?: unknown } | undefined;
  return typeof custom?.messageId === "string" ? custom.messageId : message?.id;
}

function parseStoredParts(message: ConversationMessage): unknown[] {
  const raw = (message as ConversationMessage & { partsJson?: string | null }).partsJson;
  if (!raw) return [{ type: "text", text: message.content }];
  try {
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed) ? parsed : [{ type: "text", text: message.content }];
  } catch {
    return [{ type: "text", text: message.content }];
  }
}

function hasRichAssistantUiParts(message: ThreadMessageLike) {
  return Array.isArray(message.content)
    && message.content.some((part) => part.type === "generative-ui" || part.type === "source");
}

export function conversationMessagesToAgUiThreadMessages(messages: ConversationMessage[]): ThreadMessageLike[] {
  const legacy = conversationMessagesToThreadMessages(messages);
  const legacyById = new Map(legacy.map((message) => [message.id, message]));
  const agUiMessages = messages.flatMap((message) => {
    if (message.role === "user") {
      return [{ id: message.id, role: "user", content: message.content }];
    }

    const parts = parseStoredParts(message);
    const reasoning = parts
      .filter((part): part is { type: "reasoning"; text: string } =>
        Boolean(part && typeof part === "object" && (part as { type?: unknown }).type === "reasoning" && typeof (part as { text?: unknown }).text === "string")
      )
      .map((part, index) => ({
        id: `${message.id}:reasoning:${index}`,
        role: "reasoning",
        content: part.text,
      }));
    const toolCalls = parts
      .filter((part): part is { type: "tool-call"; toolCallId?: string; toolName?: string; argsText?: string; args?: unknown; result?: unknown; isError?: boolean } =>
        Boolean(part && typeof part === "object" && (part as { type?: unknown }).type === "tool-call")
      );
    const assistant = {
      id: message.id,
      role: "assistant",
      content: message.content,
      toolCalls: toolCalls.map((part, index) => {
        const id = part.toolCallId ?? `${message.id}:tool:${index}`;
        return {
          id,
          type: "function",
          function: {
            name: part.toolName ?? "tool",
            arguments: part.argsText ?? JSON.stringify(part.args ?? {}),
          },
        };
      }),
    };
    const toolResults = toolCalls.flatMap((part, index) => {
      if (part.result === undefined) return [];
      const toolCallId = part.toolCallId ?? `${message.id}:tool:${index}`;
      return [{
        id: `${toolCallId}:result`,
        role: "tool",
        toolCallId,
        content: typeof part.result === "string" ? part.result : JSON.stringify(part.result),
        ...(part.isError ? { error: "Tool call failed" } : {}),
      }];
    });
    return [...reasoning, assistant, ...toolResults];
  });

  const converted = fromAgUiMessages(agUiMessages, { showThinking: true });
  return legacy.map((legacyMessage) => {
    if (hasRichAssistantUiParts(legacyMessage)) return legacyMessage;
    const convertedMessage = converted.find((message) => message.id === legacyMessage.id);
    return convertedMessage ? {
      ...convertedMessage,
      metadata: legacyMessage.metadata,
      createdAt: legacyMessage.createdAt,
    } : legacyById.get(legacyMessage.id) ?? legacyMessage;
  }).map(withArrayContent);
}

// AG-UI's `applyExternalMessages` stores loaded history verbatim — unlike the
// streaming path it does NOT run messages through `fromThreadMessageLike`, so
// the render client calls `message.content.map(...)` on whatever we hand it.
// `fromAgUiMessages` passes a user turn's string content straight through, which
// crashes rendering (content.map is not a function) and drops the thread to its
// empty state — i.e. "history isn't loading". Coerce any string content into a
// text part so every loaded message is renderable.
function withArrayContent(message: ThreadMessageLike): ThreadMessageLike {
  if (typeof message.content === "string") {
    return { ...message, content: [{ type: "text", text: message.content }] };
  }
  return message;
}

function metaFromDone(payload: Extract<CopilotStreamFrame, { type: "done" }>): MessageMeta {
  return {
    bundleId: payload.bundleId ?? undefined,
    toolTrace: payload.toolTrace,
    followUpQuestions: payload.followUpQuestions,
    missingData: payload.missingData,
    actionLabel: payload.actionLabel ?? undefined,
    actionPath: payload.actionPath ?? undefined,
    providerId: payload.providerId,
    modelId: payload.modelId,
    elapsedMs: payload.elapsedMs,
    toolCount: payload.toolCount,
    cachedTokens: payload.cachedTokens,
    promptTokens: payload.promptTokens,
  };
}

export function useTauriAgUiRuntime(initialConversationId?: string | null): {
  runtime: AssistantRuntime;
  latestMeta: MessageMeta | null;
  metaByMessageId: MetaByMessageId;
} {
  const queryClient = useQueryClient();
  const [latestMeta, setLatestMeta] = useState<MessageMeta | null>(null);
  const [metaByMessageId, setMetaByMessageId] = useState<MetaByMessageId>({});
  const agent = useMemo(() => new TauriAgUiAgent({
    conversationId: initialConversationId ?? null,
    onDone(payload) {
      const meta = metaFromDone(payload);
      setLatestMeta(meta);
      setMetaByMessageId((prev) => ({ ...prev, [payload.messageId]: meta }));
      void queryClient.invalidateQueries({ queryKey: ["conversations"] });
      void queryClient.invalidateQueries({ queryKey: ["conversation-messages", payload.conversationId] });
      void queryClient.invalidateQueries({ queryKey: ["action-bundles"] });
    },
    onPlan(payload) {
      // Keyed by the client-generated assistantMessageId (not onDone's persisted
      // DB messageId): the plan frame arrives before `done`, and in the live path
      // AssistantMessageWithMeta looks meta up by `message.id`, which equals this
      // assistantMessageId (it's the TEXT_MESSAGE_START messageId). Merge into any
      // existing meta rather than replacing so we don't clobber sibling fields.
      setMetaByMessageId((prev) => ({
        ...prev,
        [payload.assistantMessageId]: { ...prev[payload.assistantMessageId], plan: payload.steps },
      }));
    },
  }), [initialConversationId, queryClient]);

  const loadConversation = useCallback(async (conversationId: string | null) => {
    if (!conversationId) {
      agent.setConversationId(null);
      setMetaByMessageId({});
      return { messages: [] as ThreadMessage[] };
    }

    agent.setConversationId(conversationId);
    const messages = unwrapCommandResult(await commands.getConversationMessages(conversationId));
    const meta = buildMetaFromMessages(messages);
    setMetaByMessageId(meta);
    return {
      messages: conversationMessagesToAgUiThreadMessages(messages).map((message) => message as ThreadMessage),
    };
  }, [agent]);

  const history = useMemo<ThreadHistoryAdapter>(() => ({
    async load() {
      const loaded = await loadConversation(agent.getConversationId());
      return {
        messages: loaded.messages.map((message) => ({
          parentId: null,
          message,
        })),
      };
    },
    async append(_item: ExportedMessageRepositoryItem) {
      // streamCopilotMessage persists the user turn and assistant turn in Rust.
    },
    async update(item: ExportedMessageRepositoryItem) {
      if (item.message.role !== "user") return;
      const conversationId = agent.getConversationId();
      const messageId = backendMessageId(item.message);
      const content = textFromMessage(item.message).trim();
      if (!conversationId || !messageId || !content) return;
      unwrapCommandResult(await commands.editConversationUserMessage({
        conversationId,
        messageId,
        content,
      }));
      void queryClient.invalidateQueries({ queryKey: ["conversation-messages", conversationId] });
    },
    async delete(items: ExportedMessageRepositoryItem[]) {
      const conversationId = agent.getConversationId();
      const firstBackendId = items
        .map((item) => backendMessageId(item.message))
        .find((id): id is string => Boolean(id));
      if (!conversationId || !firstBackendId) return;
      unwrapCommandResult(await commands.deleteConversationMessagesAfter(conversationId, firstBackendId));
      void queryClient.invalidateQueries({ queryKey: ["conversation-messages", conversationId] });
      void queryClient.invalidateQueries({ queryKey: ["conversations"] });
    },
  }), [agent, loadConversation, queryClient]);

  const runtime = useAgUiRuntime({
    agent,
    showThinking: true,
    adapters: { history },
    onError(error) {
      console.error("[Copilot AG-UI] runtime error", error);
    },
    onCancel() {
      setLatestMeta(null);
    },
  });

  return {
    runtime,
    latestMeta,
    metaByMessageId,
  };
}
