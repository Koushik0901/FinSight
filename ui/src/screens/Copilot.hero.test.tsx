import { describe, it, expect, vi, beforeEach } from "vitest";
import { forwardRef } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import Copilot from "./Copilot";
import { createWrapper } from "../test-utils";

// ── Mock external hooks / modules ─────────────────────────────────────────────
// Same proven pattern as Copilot.test.tsx (rendering <Copilot /> requires
// mocking @assistant-ui/react itself, not just the runtime hook).

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

vi.mock("../components/copilot/agUi/TauriAgUiRuntime", () => ({
  useTauriAgUiRuntime: vi.fn(() => runtimeStub),
}));

vi.mock("../components/copilot/agUi/featureFlag", () => ({
  isCopilotAgUiRuntimeEnabled: () => false,
}));

// Grounding-stats data sources — real counts, per the task: 42 transactions,
// 1 account. Must NOT match the old hardcoded mockup number (1,247).
const accountsMock = vi.hoisted(() => ({ list: [{ id: "a1" }] as unknown[] }));
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: accountsMock.list, isLoading: false, error: null })),
}));
const txnCountMock = vi.hoisted(() => ({ count: 42 }));
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
  accountsMock.list = [{ id: "a1" }];
  txnCountMock.count = 42;
});

describe("Copilot hero", () => {
  it("renders the hero shell with real grounding stats, not hardcoded mockup numbers", async () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(await screen.findByText(/42 transaction/i)).toBeInTheDocument();
    expect(screen.queryByText(/1,247 transaction/i)).not.toBeInTheDocument();
  });

  it("renders suggestion chips instead of the old prompt-card grid", async () => {
    render(<Copilot />, { wrapper: createWrapper() });
    const chip = await screen.findByRole("button", { name: /Plan next month's budget/i });
    expect(chip.className).toContain("cp-hero-chip");
  });

  it("wraps the empty state in the new .cp-hero shell structure", async () => {
    const { container } = render(<Copilot />, { wrapper: createWrapper() });
    await waitFor(() => expect(screen.getByText(/42 transaction/i)).toBeInTheDocument());
    expect(container.querySelector(".cp-hero")).toBeInTheDocument();
    expect(container.querySelector(".cp-hero-glow")).toBeInTheDocument();
    expect(container.querySelector(".cp-hero-inner")).toBeInTheDocument();
    expect(container.querySelector(".copilot-prompt-card")).not.toBeInTheDocument();
  });

  it("is still honest about an empty workspace (no fabricated data)", async () => {
    accountsMock.list = [];
    txnCountMock.count = 0;
    render(<Copilot />, { wrapper: createWrapper() });
    await waitFor(() =>
      expect(screen.getByText(/No financial data imported yet/i)).toBeInTheDocument()
    );
  });
});
