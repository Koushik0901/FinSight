// Re-export the typed commands and types from the generated bindings.
// All Tauri IPC access in the UI should route through this module so the
// bindings file remains a generated implementation detail.
export * from "./bindings";
import type { AgentResponseBlock } from "./bindings";

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
  actionLabel: string | null;
  actionPath: string | null;
};

export type CopilotResponseBlock = AgentResponseBlock;

export type CopilotStreamFrame =
  | { type: "text"; conversationId: string; runId: string; delta: string }
  | { type: "reasoning"; conversationId: string; runId: string; text: string }
  | {
      type: "toolCallStart";
      conversationId: string;
      runId: string;
      toolCallId: string;
      toolName: string;
      args: Record<string, unknown>;
    }
  | {
      type: "toolCallResult";
      conversationId: string;
      runId: string;
      toolCallId: string;
      result: unknown;
      isError: boolean;
    }
  | {
      type: "responseBlock";
      conversationId: string;
      runId: string;
      blockId: string;
      block: CopilotResponseBlock;
    }
  | { type: "source"; conversationId: string; runId: string; sourceId: string; title: string }
  | {
      type: "usage";
      conversationId: string;
      runId: string;
      providerId: string;
      modelId: string;
      elapsedMs: number;
      toolCount: number;
    }
  | {
      type: "done";
      conversationId: string;
      runId: string;
      messageId: string;
      bundleId: string | null;
      toolTrace: string[];
      followUpQuestions: string[];
      actionLabel: string | null;
      actionPath: string | null;
      providerId: string;
      modelId: string;
      elapsedMs: number;
      toolCount: number;
    }
  | { type: "error"; conversationId: string; runId: string; code: string; message: string };
