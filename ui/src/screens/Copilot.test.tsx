import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Copilot from "./Copilot";
import { createWrapper } from "../test-utils";

// ── Mock external hooks / modules ─────────────────────────────────────────────

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

vi.mock("../components/copilot/TauriRuntime", () => ({
  useTauriCopilotRuntime: vi.fn(() => ({
    runtime: {
      thread: {
        composer: { setText: vi.fn() },
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
  })),
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
      Messages: () => null,
      Empty: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    },
    ComposerPrimitive: {
      Root: ({ children, className }: { children: React.ReactNode; className?: string }) => (
        <form className={className} onSubmit={(e) => e.preventDefault()}>{children}</form>
      ),
      Input: ({ placeholder, className, ...rest }: React.TextareaHTMLAttributes<HTMLTextAreaElement>) => (
        <textarea placeholder={placeholder} className={className} {...rest} />
      ),
      Send: ({ children, className }: { children: React.ReactNode; className?: string }) => (
        <button type="submit" className={className}>{children}</button>
      ),
      Cancel: ({ children, className }: { children: React.ReactNode; className?: string }) => (
        <button type="button" className={className}>{children}</button>
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
    Tools: vi.fn(() => ({})),
    groupPartByType: vi.fn(() => vi.fn()),
    useMessage: vi.fn(() => ({ id: "msg-1", role: "assistant", status: { type: "complete" }, content: [] })),
    useMessageTiming: vi.fn(() => null),
    useAui: vi.fn(() => ({})),
    useThreadRuntime: vi.fn(() => ({
      composer: { setText: vi.fn() },
      reset: vi.fn(),
    })),
    useThread: vi.fn(() => ({
      composer: { setText: vi.fn() },
      isRunning: false,
    })),
  };
});

vi.mock("@assistant-ui/react-markdown", () => ({
  MarkdownTextPrimitive: () => null,
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
});

describe("Copilot screen — rendering", () => {
  it("renders the heading", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByRole("heading", { name: /copilot/i })).toBeInTheDocument();
  });

  it("renders the conversation sidebar", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByText("Conversations")).toBeInTheDocument();
  });

  it("renders the New conversation button in sidebar", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    const newBtn = screen.getByTitle("New conversation");
    expect(newBtn).toBeInTheDocument();
  });

  it("renders Chat and Memory tab buttons in the header", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByRole("button", { name: /Chat/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Memory/i })).toBeInTheDocument();
  });

  it("shows suggested prompts when no conversation is active", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByText(/Plan next month's budget/i)).toBeInTheDocument();
    expect(screen.getByText(/What can I cut to improve my savings rate/i)).toBeInTheDocument();
  });

  it("shows empty sidebar message when no conversations exist", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByText(/No conversations yet/i)).toBeInTheDocument();
  });
});

describe("Copilot screen — conversation list", () => {
  it("shows conversations from the sidebar with grouping", async () => {
    const { useConversations } = await import("../api/hooks/copilotChat");
    vi.mocked(useConversations).mockReturnValue({
      data: [
        {
          id: "c1",
          title: "Budget analysis",
          messageCount: 3,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    } as ReturnType<typeof useConversations>);

    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByText("Budget analysis")).toBeInTheDocument();
  });

  it("filters conversations via search", async () => {
    const { useConversations } = await import("../api/hooks/copilotChat");
    vi.mocked(useConversations).mockReturnValue({
      data: [
        {
          id: "c1",
          title: "Budget analysis",
          messageCount: 2,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
        {
          id: "c2",
          title: "Debt payoff plan",
          messageCount: 1,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    } as ReturnType<typeof useConversations>);

    render(<Copilot />, { wrapper: createWrapper() });
    const searchInput = screen.getByPlaceholderText("Search…");
    fireEvent.change(searchInput, { target: { value: "budget" } });

    expect(screen.getByText("Budget analysis")).toBeInTheDocument();
    expect(screen.queryByText("Debt payoff plan")).not.toBeInTheDocument();
  });
});

describe("Copilot screen — new conversation", () => {
  it("creates a new conversation when the new button is clicked", async () => {
    const { useCreateConversation } = await import("../api/hooks/copilotChat");
    const mockCreate = vi.fn().mockResolvedValue("new-conv-id");
    vi.mocked(useCreateConversation).mockReturnValue({
      mutateAsync: mockCreate,
      isPending: false,
    } as unknown as ReturnType<typeof useCreateConversation>);

    render(<Copilot />, { wrapper: createWrapper() });
    const newBtn = screen.getByTitle("New conversation");
    fireEvent.click(newBtn);

    await waitFor(() => {
      expect(mockCreate).toHaveBeenCalled();
    });
  });

  it("creates a new conversation when a prompt card is clicked", async () => {
    const { useCreateConversation } = await import("../api/hooks/copilotChat");
    const mockCreate = vi.fn().mockResolvedValue("new-conv-id");
    vi.mocked(useCreateConversation).mockReturnValue({
      mutateAsync: mockCreate,
      isPending: false,
    } as unknown as ReturnType<typeof useCreateConversation>);

    render(<Copilot />, { wrapper: createWrapper() });
    const promptCard = screen.getByText(/Plan next month's budget/i);
    fireEvent.click(promptCard);

    await waitFor(() => {
      expect(mockCreate).toHaveBeenCalled();
    });
  });
});

describe("Copilot screen — memory tab", () => {
  it("switches to Memory tab when clicked", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /Memory/i }));
    expect(screen.queryByText(/Start a conversation/i)).not.toBeInTheDocument();
  });

  it("shows empty memory state when there are no memories", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /Memory/i }));
    expect(screen.getByText(/No saved memory yet/i)).toBeInTheDocument();
  });
});

describe("Copilot screen — conversation selection", () => {
  it("selecting a conversation shows the thread with composer", async () => {
    const { useConversations } = await import("../api/hooks/copilotChat");
    vi.mocked(useConversations).mockReturnValue({
      data: [
        {
          id: "c1",
          title: "My first conversation",
          messageCount: 2,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ],
    } as ReturnType<typeof useConversations>);

    render(<Copilot />, { wrapper: createWrapper() });
    const convBtn = screen.getByText("My first conversation");
    fireEvent.click(convBtn);

    await waitFor(() => {
      // Composer input should be visible
      expect(
        screen.getByPlaceholderText(/Ask your financial analyst anything/i)
      ).toBeInTheDocument();
    });
  });
});

describe("Copilot screen — sessionStorage prefill", () => {
  it("reads copilot.prefill and creates a conversation", async () => {
    const { useCreateConversation } = await import("../api/hooks/copilotChat");
    const mockCreate = vi.fn().mockResolvedValue("prefill-conv-id");
    vi.mocked(useCreateConversation).mockReturnValue({
      mutateAsync: mockCreate,
      isPending: false,
    } as unknown as ReturnType<typeof useCreateConversation>);

    sessionStorage.setItem("copilot.prefill", "Auto-filled question");
    render(<Copilot />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(sessionStorage.getItem("copilot.prefill")).toBeNull();
    });
  });
});
