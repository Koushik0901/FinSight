import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  AssistantRuntimeProvider,
  ThreadPrimitive,
  MessagePrimitive,
} from "@assistant-ui/react";

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

function Wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return createElement(QueryClientProvider, { client }, children);
}

// Minimal faithful reproduction of CopilotThread's message rendering: an
// empty-state gate on `thread.isEmpty` plus text parts, exactly like the app.
function Thread() {
  const { runtime } = useTauriAgUiRuntime("conv-1");
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ThreadPrimitive.Root>
        <ThreadPrimitive.Empty>
          <div data-testid="empty-state">No messages</div>
        </ThreadPrimitive.Empty>
        <ThreadPrimitive.Messages
          components={{
            Message: () => (
              <MessagePrimitive.Root>
                <MessagePrimitive.Parts>
                  {({ part }) => (part.type === "text" ? <span>{part.text}</span> : null)}
                </MessagePrimitive.Parts>
              </MessagePrimitive.Root>
            ),
          }}
        />
      </ThreadPrimitive.Root>
    </AssistantRuntimeProvider>
  );
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
      partsJson: JSON.stringify([
        { type: "reasoning", text: "Reviewed the snapshot." },
        { type: "source", sourceType: "url", id: "src-0", url: "finsight://snapshot", title: "Snapshot" },
        { type: "tool-call", toolCallId: "tool-0", toolName: "get_financial_snapshot", args: {}, argsText: "{}", result: { ok: true } },
        { type: "text", text: "Here is a budget plan." },
      ]),
    },
  ];
}

describe("Copilot AG-UI thread renders loaded history", () => {
  beforeEach(() => {
    for (const key of Object.keys(listeners)) delete listeners[key];
    vi.clearAllMocks();
    getConversationMessages.mockResolvedValue({ status: "ok", data: storedConversation() });
  });

  it("shows persisted user + assistant text in the rendered thread (not the empty state)", async () => {
    render(<Thread />, { wrapper: Wrapper });

    // The visible message text from the loaded conversation must appear...
    await waitFor(() => expect(screen.getByText("Here is a budget plan.")).toBeInTheDocument());
    expect(screen.getByText("Plan next month's budget")).toBeInTheDocument();
    // ...and the empty-state must NOT be shown.
    expect(screen.queryByTestId("empty-state")).not.toBeInTheDocument();
  });
});
