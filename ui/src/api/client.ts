// Re-export the typed commands and types from the generated bindings.
// All Tauri IPC access in the UI should route through this module so the
// bindings file remains a generated implementation detail.
export * from "./bindings";
import type { AgentResponseBlock, AppError, MissingDataItem, Result } from "./bindings";

/**
 * Await a generated command call and turn the `Result` envelope into either
 * the payload or a thrown `Error` — the single shared replacement for the
 * `if (result.status === "error") throw …` boilerplate that used to be
 * copy-pasted at every call site.
 */
export async function unwrap<T>(call: Promise<Result<T, AppError>>): Promise<T> {
  const result = await call;
  if (result.status === "error") throw new Error(result.error.message);
  return result.data;
}

/** Value-based sibling of [`unwrap`] for code already holding a `Result`. */
export function unwrapResult<T>(result: Result<T, AppError>): T {
  if (result.status === "error") throw new Error(result.error.message);
  return result.data;
}

// ── Tauri event payload types (emitted by Rust, not auto-generated) ───────────

export type CopilotTokenPayload = {
  conversationId: string;
  runId: string;
  token: string;
};

export type CopilotDonePayload = {
  conversationId: string;
  runId: string;
  messageId: string;
  bundleId: string | null;
  toolTrace: string[];
  followUpQuestions: string[];
  missingData: MissingDataItem[];
  actionLabel: string | null;
  actionPath: string | null;
};

export type CopilotResponseBlock = AgentResponseBlock;

export type CopilotStreamFrameMeta = {
  threadId?: string;
  assistantMessageId?: string;
  reasoningMessageId?: string;
  toolResultMessageId?: string;
  parentMessageId?: string | null;
  sequenceNumber?: number;
};

export type CopilotStreamFrame =
  | ({ type: "text"; conversationId: string; runId: string; delta: string } & CopilotStreamFrameMeta)
  | ({ type: "reasoning"; conversationId: string; runId: string; text: string } & CopilotStreamFrameMeta)
  | ({
      type: "toolCallStart";
      conversationId: string;
      runId: string;
      toolCallId: string;
      toolName: string;
      args: Record<string, unknown>;
    } & CopilotStreamFrameMeta)
  | ({
      type: "toolCallResult";
      conversationId: string;
      runId: string;
      toolCallId: string;
      result: unknown;
      isError: boolean;
    } & CopilotStreamFrameMeta)
  | ({
      type: "responseBlock";
      conversationId: string;
      runId: string;
      blockId: string;
      block: CopilotResponseBlock;
    } & CopilotStreamFrameMeta)
  | ({ type: "source"; conversationId: string; runId: string; sourceId: string; title: string } & CopilotStreamFrameMeta)
  | ({ type: "plan"; conversationId: string; runId: string; steps: string[] } & CopilotStreamFrameMeta)
  | ({
      type: "usage";
      conversationId: string;
      runId: string;
      providerId: string;
      modelId: string;
      elapsedMs: number;
      toolCount: number;
      cachedTokens?: number;
      promptTokens?: number;
    } & CopilotStreamFrameMeta)
  | ({
      type: "done";
      conversationId: string;
      runId: string;
      messageId: string;
      bundleId: string | null;
      toolTrace: string[];
      followUpQuestions: string[];
      missingData: MissingDataItem[];
      actionLabel: string | null;
      actionPath: string | null;
      providerId: string;
      modelId: string;
      elapsedMs: number;
      toolCount: number;
      cachedTokens?: number;
      promptTokens?: number;
    } & CopilotStreamFrameMeta)
  | ({ type: "error"; conversationId: string; runId: string; code: string; message: string } & CopilotStreamFrameMeta);
