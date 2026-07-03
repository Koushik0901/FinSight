import { describe, it, expect, vi, beforeEach } from "vitest";
import { forwardRef } from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Copilot from "./Copilot";
import { createWrapper } from "../test-utils";

// ── Mock external hooks / modules ─────────────────────────────────────────────

const assistantUiMocks = vi.hoisted(() => ({
  threadList: [] as Array<{ id: string; title: string }>,
  switchToNewThread: vi.fn(),
  setComposerText: vi.fn(),
}));

vi.mock("../api/hooks/copilot", () => ({
  useActionBundles: vi.fn(() => ({ data: [], isLoading: false })),
  useActionBundle: vi.fn(() => ({ data: null, isLoading: false })),
  useApproveActionItem: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useRejectActionItem: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useExecutionLog: vi.fn(() => ({ data: [], isLoading: false })),
}));

vi.mock("../api/hooks/agentMemory", () => ({
  useAgentMemory: vi.fn(() => ({ data: [] })),
  useForgetAgentMemory: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

vi.mock("../api/hooks/copilotChat", () => ({
  useConversations: vi.fn(() => ({ data: [] })),
  useConversationMessages: vi.fn(() => ({ data: [] })),
  useCreateConversation: vi.fn(() => ({
    mutateAsync: vi.fn().mockResolvedValue("test-conv-id"),
    isPending: false,
  })),
  useDeleteConversation: vi.fn(() => ({
    mutateAsync: vi.fn(),
    isPending: false,
  })),
}));

const runtimeStub = {
  runtime: {
    thread: {
      composer: { setText: assistantUiMocks.setComposerText },
      reset: vi.fn(),
      getState: vi.fn(() => ({ isRunning: false })),
    },
    threads: {},
    registerModelContextProvider: vi.fn(),
  },
  messages: [],
  isRunning: false,
  latestMeta: null,
  metaByMessageId: {},
};

vi.mock("../components/copilot/TauriRuntime", () => ({
  useTauriCopilotRuntime: vi.fn(() => runtimeStub),
}));

// AG-UI is the default runtime (Phase 5B); mock it so the screen renders
// deterministically without a live Tauri bridge.
vi.mock("../components/copilot/agUi/TauriAgUiRuntime", () => ({
  useTauriAgUiRuntime: vi.fn(() => runtimeStub),
}));

// Grounding-stats data sources. Defaults to an empty workspace (honest
// "no data imported" state); individual tests override as needed.
const accountsMock = vi.hoisted(() => ({ list: [] as unknown[] }));
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: accountsMock.list })),
}));
const txnCountMock = vi.hoisted(() => ({ count: 0 }));
vi.mock("../api/client", () => ({
  commands: {
    getTransactionCount: vi.fn(async () => ({ status: "ok", data: txnCountMock.count })),
  },
}));

// Mock assistant-ui to avoid complex runtime setup in tests
vi.mock("@assistant-ui/react", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@assistant-ui/react")>();
  return {
    ...actual,
    AssistantRuntimeProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
    ThreadPrimitive: {
      Root: ({ children, className }: { children: React.ReactNode; className?: string }) => (
        <div className={className}>{children}</div>
      ),
      Viewport: ({ children, className }: { children: React.ReactNode; className?: string }) => (
        <div className={className}>{children}</div>
      ),
      ViewportFooter: ({ children, className }: { children: React.ReactNode; className?: string }) => (
        <div className={className}>{children}</div>
      ),
      Messages: () => null,
      Empty: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
      ScrollToBottom: ({ children, className, ...rest }: { children: React.ReactNode; className?: string }) => (
        <button type="button" className={className} {...rest}>{children}</button>
      ),
    },
    ComposerPrimitive: {
      Root: ({ children, className }: { children: React.ReactNode; className?: string }) => (
        <form className={className} onSubmit={(e) => e.preventDefault()}>{children}</form>
      ),
      Input: forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
        ({ placeholder, className, ...rest }, ref) => (
          <textarea ref={ref} placeholder={placeholder} className={className} {...rest} />
        )
      ),
      Send: ({ children, className, ...rest }: { children: React.ReactNode; className?: string }) => (
        <button type="submit" className={className} {...rest}>{children}</button>
      ),
      Cancel: ({ children, className, ...rest }: { children: React.ReactNode; className?: string }) => (
        <button type="button" className={className} {...rest}>{children}</button>
      ),
    },
    MessagePrimitive: {
      Root: ({ children, className }: { children: React.ReactNode; className?: string }) => (
        <div className={className}>{children}</div>
      ),
      Content: () => null,
      Parts: () => null,
      GroupedParts: () => null,
      GenerativeUI: () => null,
      Quote: ({ children, className }: { children: React.ReactNode; className?: string }) => <blockquote className={className}>{children}</blockquote>,
      Error: ({ children, className }: { children: React.ReactNode; className?: string }) => <div className={className}>{children}</div>,
    },
    ActionBarPrimitive: {
      Root: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
      Copy: ({ children, className }: { children: React.ReactNode; className?: string }) => <button className={className}>{children}</button>,
      Edit: ({ children, className }: { children: React.ReactNode; className?: string }) => <button className={className}>{children}</button>,
      Reload: ({ children, className }: { children: React.ReactNode; className?: string }) => <button className={className}>{children}</button>,
    },
    BranchPickerPrimitive: {
      Root: ({ children, className }: { children: React.ReactNode; className?: string }) => <div className={className}>{children}</div>,
      Previous: ({ children, className }: { children: React.ReactNode; className?: string }) => <button className={className}>{children}</button>,
      Next: ({ children, className }: { children: React.ReactNode; className?: string }) => <button className={className}>{children}</button>,
      Number: () => <span>1</span>,
      Count: () => <span>1</span>,
    },
    ThreadListPrimitive: {
      Root: ({ children, className }: { children: React.ReactNode; className?: string }) => <div className={className}>{children}</div>,
      New: ({ children, className, title, onClick }: { children: React.ReactNode; className?: string; title?: string; onClick?: () => void }) => (
        <button type="button" className={className} title={title} onClick={() => { onClick?.(); assistantUiMocks.switchToNewThread(); }}>{children}</button>
      ),
      Items: ({ children }: { children?: (value: { threadListItem: { id: string; title: string; lastMessageAt?: Date } }) => React.ReactNode }) =>
        assistantUiMocks.threadList.length === 0 ? (
          <div>No conversations yet.</div>
        ) : (
          <div>
            {assistantUiMocks.threadList.map((thread) => (
              <div key={thread.id}>
                {children
                  ? children({ threadListItem: { ...thread, lastMessageAt: new Date("2026-06-30") } })
                  : <button type="button">{thread.title}</button>}
              </div>
            ))}
          </div>
        ),
      LoadMore: ({ children, className }: { children: React.ReactNode; className?: string }) => <button type="button" className={className}>{children}</button>,
    },
    ThreadListItemPrimitive: {
      Root: ({ children, className }: { children: React.ReactNode; className?: string }) => <div className={className}>{children}</div>,
      Trigger: ({ children, className, onClick }: { children: React.ReactNode; className?: string; onClick?: () => void }) => <button type="button" className={className} onClick={onClick}>{children}</button>,
      Title: ({ fallback }: { fallback?: string }) => <span>{fallback}</span>,
      Delete: ({ children, className }: { children: React.ReactNode; className?: string }) => <button type="button" className={className}>{children}</button>,
    },
    AuiIf: ({
      children,
      condition,
    }: {
      children: React.ReactNode;
      condition?: (state: { thread: { isEmpty: boolean } }) => boolean;
    }) => {
      const state = { thread: { isEmpty: true } };
      return condition && !condition(state) ? null : <>{children}</>;
    },
    ErrorPrimitive: {
      Message: () => <span>Copilot error</span>,
    },
    Tools: vi.fn(() => ({})),
    groupPartByType: vi.fn(() => vi.fn()),
    useMessage: vi.fn(() => ({ id: "msg-1", role: "assistant", status: { type: "complete" }, content: [] })),
    useMessageTiming: vi.fn(() => null),
    useAui: vi.fn(() => ({
      threads: () => ({
        switchToNewThread: assistantUiMocks.switchToNewThread,
      }),
    })),
    useThreadRuntime: vi.fn(() => ({
      composer: { setText: assistantUiMocks.setComposerText },
      reset: vi.fn(),
    })),
    useThread: vi.fn(() => ({
      composer: { setText: vi.fn() },
      isRunning: false,
      isEmpty: true,
    })),
  };
});

vi.mock("@assistant-ui/react-streamdown", () => ({
  StreamdownTextPrimitive: () => null,
}));

vi.mock("@streamdown/code", () => ({
  code: { name: "shiki", type: "code-highlighter" },
}));

vi.mock("@streamdown/cjk", () => ({
  cjk: { name: "cjk", type: "cjk" },
}));

vi.mock("@streamdown/math", () => ({
  math: { name: "katex", type: "math" },
}));

vi.mock("@streamdown/mermaid", () => ({
  mermaid: { name: "mermaid", type: "diagram", language: "mermaid" },
}));

vi.mock("sonner", () => ({
  toast: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
  }),
}));

// ── Tests ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  sessionStorage.clear();
  vi.clearAllMocks();
  assistantUiMocks.threadList = [];
  accountsMock.list = [];
  txnCountMock.count = 0;
});

describe("Copilot screen — rendering", () => {
  it("renders the FinSight Copilot shell without assistant-ui demo chrome", () => {
    const { container } = render(<Copilot />, { wrapper: createWrapper() });
    expect(container.querySelector(".copilot-finsight-chat")).toBeInTheDocument();
    expect(container.querySelector(".copilot-playground-clone")).not.toBeInTheDocument();
    expect(screen.queryByText("assistant-ui")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /AI Builder/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /UI Builder/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Templates/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /GitHub/i })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /^Copilot$/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /New thread/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /what should we work through/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^History$/i })).toBeInTheDocument();
  });

  it("shows suggested prompts when no conversation is active", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByText(/Plan next month's budget/i)).toBeInTheDocument();
    expect(screen.getByText(/Improve my savings rate/i)).toBeInTheDocument();
    expect(screen.getByText(/Clean up uncategorized transactions/i)).toBeInTheDocument();
  });

  it("is honest about an empty workspace (no fabricated data)", async () => {
    render(<Copilot />, { wrapper: createWrapper() });
    await waitFor(() =>
      expect(screen.getByText(/No financial data imported yet/i)).toBeInTheDocument()
    );
  });

  it("shows real grounded counts when data exists", async () => {
    accountsMock.list = [{ id: "a" }, { id: "b" }];
    txnCountMock.count = 1247;
    render(<Copilot />, { wrapper: createWrapper() });
    await waitFor(() => expect(screen.getByText(/1,247 transactions/i)).toBeInTheDocument());
    expect(screen.getByText(/2 accounts/i)).toBeInTheDocument();
    expect(screen.getByText(/100% local/i)).toBeInTheDocument();
  });

  it("renders the base composer and scroll control", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByPlaceholderText(/Ask FinSight to plan/i)).toBeInTheDocument();
    expect(screen.queryByText("FinSight Copilot")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Send message/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Scroll to bottom/i })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /New thread/i })).toBeInTheDocument();
    expect(screen.queryByText(/Powered by/i)).not.toBeInTheDocument();
    expect(screen.queryByText("0 (0%)")).not.toBeInTheDocument();
  });
});

describe("Copilot screen — new conversation", () => {
  it("prefills the composer when a prompt card is clicked", async () => {
    render(<Copilot />, { wrapper: createWrapper() });
    const promptCard = screen.getByText(/Plan next month's budget/i);
    fireEvent.click(promptCard);

    await waitFor(() => {
      expect(assistantUiMocks.setComposerText).toHaveBeenCalled();
    });
  });
});

describe("Copilot screen — sessionStorage prefill", () => {
  it("reads copilot.prefill and starts a new assistant-ui thread", async () => {
    sessionStorage.setItem("copilot.prefill", "Auto-filled question");
    render(<Copilot />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(sessionStorage.getItem("copilot.prefill")).toBeNull();
      expect(assistantUiMocks.switchToNewThread).toHaveBeenCalled();
    });
  });
});
