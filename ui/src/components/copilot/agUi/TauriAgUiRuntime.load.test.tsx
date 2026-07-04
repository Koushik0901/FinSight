import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

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

const getConversationMessages = vi.fn();

vi.mock("../../../api/client", () => ({
  commands: {
    getConversationMessages: (...args: unknown[]) => getConversationMessages(...args),
    createConversation: vi.fn(async () => ({ status: "ok", data: "conv-1" })),
    streamCopilotMessage: vi.fn(async () => ({ status: "ok", data: "conv-1" })),
  },
}));

import { useTauriAgUiRuntime } from "./TauriAgUiRuntime";

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return createElement(QueryClientProvider, { client }, children);
}

function storedConversation() {
  const base = {
    conversationId: "conv-1",
    toolTrace: null,
    actionBundleId: null,
    branchParentId: null,
    runStatus: "completed" as const,
    agUiMetadataJson: null,
    createdAt: "2026-07-01T00:00:00Z",
  };
  return [
    { ...base, id: "u1", role: "user" as const, content: "Plan next month's budget", partsJson: null },
    {
      ...base,
      id: "a1",
      role: "assistant" as const,
      content: "Here is a budget plan.",
      // Mirrors the REAL persisted shape: reasoning + source + tool-call + text.
      // The `source` part makes hasRichAssistantUiParts() true → legacy path.
      partsJson: JSON.stringify([
        { type: "reasoning", text: "Reviewed the snapshot." },
        { type: "source", sourceType: "url", id: "src-0", url: "finsight://snapshot", title: "Snapshot" },
        { type: "tool-call", toolCallId: "tool-0", toolName: "get_financial_snapshot", args: {}, argsText: "{}", result: { ok: true } },
        { type: "text", text: "Here is a budget plan." },
      ]),
    },
  ];
}

describe("useTauriAgUiRuntime history load", () => {
  beforeEach(() => {
    for (const key of Object.keys(listeners)) delete listeners[key];
    vi.clearAllMocks();
    getConversationMessages.mockResolvedValue({ status: "ok", data: storedConversation() });
  });

  it("loads persisted messages into the thread when mounted with a conversationId", async () => {
    const { result } = renderHook(() => useTauriAgUiRuntime("conv-1"), { wrapper });

    // (1) load fired against the right conversation
    await waitFor(() => expect(getConversationMessages).toHaveBeenCalledWith("conv-1"));

    // (2) the loaded messages actually reached the rendered thread
    await waitFor(() => {
      const messages = result.current.runtime.thread.getState().messages;
      expect(messages).toHaveLength(2);
    });

    const messages = result.current.runtime.thread.getState().messages;
    expect(messages[0]?.role).toBe("user");
    expect(messages[1]?.role).toBe("assistant");

    // Renderability: the assistant message must be a normalized ThreadMessage
    // whose content is a parts array carrying the visible text — a raw string
    // or empty content would render as a blank bubble in <Thread>.
    const assistant = messages[1];
    expect(Array.isArray(assistant?.content)).toBe(true);
    const textParts = (assistant?.content as Array<{ type: string; text?: string }>)
      .filter((part) => part.type === "text")
      .map((part) => part.text);
    expect(textParts.join(" ")).toContain("Here is a budget plan.");
    // Normalized messages carry a createdAt Date and a status — external-store
    // rendering relies on these being present.
    expect(assistant?.createdAt).toBeInstanceOf(Date);
  });
});
