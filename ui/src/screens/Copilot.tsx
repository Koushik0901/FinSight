/**
 * Copilot screen — full ChatGPT-style threaded AI chat.
 *
 * Architecture:
 *   • Left sidebar: persistent conversation list grouped by Today / This Week / Earlier
 *   • Right area: @assistant-ui/react Thread driven by TauriRuntime (ExternalStoreRuntime)
 *   • Streaming via copilot-token / copilot-done Tauri events (simulated word-by-word)
 *   • Action-item approval preserved inline below assistant bubbles
 */
import { useState, useEffect, useRef, useCallback, Component } from "react";
import type { ReactNode, ErrorInfo } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  AssistantRuntimeProvider,
  ThreadPrimitive,
  MessagePrimitive,
  ComposerPrimitive,
  ActionBarPrimitive,
  BranchPickerPrimitive,
  Tools,
  groupPartByType,
  useMessage,
  useMessageTiming,
  useAui,
  useThreadRuntime,
  useThread,
} from "@assistant-ui/react";
import { MarkdownTextPrimitive } from "@assistant-ui/react-markdown";
import "@assistant-ui/react-markdown/styles/dot.css";
import * as I from "../components/Icons";
import Badge from "../components/Badge";
import Button from "../components/Button";
import {
  useConversations,
  useCreateConversation,
  useDeleteConversation,
} from "../api/hooks/copilotChat";
import {
  useApproveActionItem,
  useRejectActionItem,
  useActionBundle,
} from "../api/hooks/copilot";
import { useAgentMemory, useForgetAgentMemory } from "../api/hooks/agentMemory";
import { useTauriCopilotRuntime, type MessageMeta } from "../components/copilot/TauriRuntime";
import {
  copilotToolkit,
  generativeUIComponents,
} from "../components/copilot/renderers";
import type { AgentMemory, ConversationSummary } from "../api/client";
import { invoke } from "@tauri-apps/api/core";
import type { ExecutionSummary } from "../api/client";

// ── Constants ────────────────────────────────────────────────────────────────

const SUGGESTED_PROMPTS = [
  { icon: "📊", label: "Plan next month's budget" },
  { icon: "💰", label: "How much should I save toward each goal?" },
  { icon: "✂️", label: "What can I cut to improve my savings rate?" },
  { icon: "📈", label: "Explain my spending this month" },
  { icon: "🧹", label: "Clean up uncategorized transactions" },
  { icon: "⚠️", label: "What financial risks am I facing?" },
  { icon: "🏦", label: "Can I afford a $2,000 expense right now?" },
  { icon: "❄️", label: "Create a plan to pay off debt faster" },
];

// ── Action item helpers ───────────────────────────────────────────────────────

function actionKindLabel(kind: string): string {
  const labels: Record<string, string> = {
    set_budget: "Set budget",
    update_goal_monthly: "Update goal contribution",
    update_goal_target: "Update goal target",
    set_transaction_category: "Categorize transaction",
    set_transaction_flag: "Flag transaction",
    create_rule: "Create category rule",
    save_scenario: "Save scenario",
    generate_report: "Generate report",
  };
  return labels[kind] ?? kind;
}

// ── ActionBundlePanel ─────────────────────────────────────────────────────────

function ActionBundlePanel({ bundleId }: { bundleId: string }) {
  const { data: bundle } = useActionBundle(bundleId);
  const approve = useApproveActionItem();
  const reject = useRejectActionItem();
  const qc = useQueryClient();
  const [isExecuting, setIsExecuting] = useState(false);
  const [executed, setExecuted] = useState(false);

  if (!bundle || bundle.items.length === 0) return null;

  const pendingItems = bundle.items.filter((i) => i.status === "pending");
  const approvedItems = bundle.items.filter((i) => i.status === "approved");
  const canExecute = approvedItems.length > 0 && !executed;

  const handleExecute = async () => {
    setIsExecuting(true);
    try {
      const raw = await invoke<ExecutionSummary>("execute_action_bundle", {
        bundleId: bundle.id,
      });
      setExecuted(true);
      await qc.invalidateQueries({ queryKey: ["action-bundles"] });
      if (raw.failed === 0) {
        toast.success(`${raw.succeeded} action${raw.succeeded !== 1 ? "s" : ""} applied`);
      } else {
        toast.error(`${raw.failed} failed`, { description: `${raw.succeeded} succeeded` });
      }
    } catch (e) {
      toast.error("Execution failed", { description: String(e) });
    } finally {
      setIsExecuting(false);
    }
  };

  return (
    <div className="stack stack-sm" style={{ marginTop: 14 }}>
      <p className="eyebrow" style={{ margin: 0 }}>
        {bundle.items.length} proposed action{bundle.items.length !== 1 ? "s" : ""}
        {pendingItems.length > 0 && (
          <span className="muted" style={{ marginLeft: 8, fontSize: 11 }}>
            · {pendingItems.length} awaiting review
          </span>
        )}
      </p>

      {bundle.items.map((item) => {
        let payload: Record<string, unknown> = {};
        try { payload = JSON.parse(item.payloadJson) as Record<string, unknown>; } catch { /* ok */ }
        const isPending = item.status === "pending";

        return (
          <div
            key={item.id}
            style={{
              padding: "10px 14px",
              background: "var(--elevated)",
              borderRadius: 8,
              border: "1px solid var(--line)",
              opacity: item.status === "rejected" ? 0.5 : 1,
            }}
          >
            <div style={{ display: "flex", gap: 10, alignItems: "flex-start" }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
                  <span style={{ fontSize: 13, fontWeight: 600 }}>
                    {actionKindLabel(item.actionKind)}
                  </span>
                  <Badge tone={item.confidence >= 0.8 ? "positive" : "warning"}>
                    {Math.round(item.confidence * 100)}%
                  </Badge>
                  {!isPending && (
                    <Badge
                      tone={
                        item.status === "approved" || item.status === "executed"
                          ? "positive"
                          : "negative"
                      }
                      dot
                    >
                      {item.status}
                    </Badge>
                  )}
                </div>
                <p className="muted" style={{ margin: "4px 0 0", fontSize: 12.5, lineHeight: 1.4 }}>
                  {item.rationale}
                </p>
                {Object.keys(payload).length > 0 && (
                  <div style={{
                    marginTop: 6,
                    padding: "3px 7px",
                    background: "var(--bg)",
                    borderRadius: 4,
                    fontSize: 11.5,
                    fontFamily: "var(--mono)",
                    color: "var(--ink-faint)",
                    wordBreak: "break-all",
                  }}>
                    {Object.entries(payload)
                      .filter(([k]) => k !== "params")
                      .map(([k, v]) => `${k}: ${String(v)}`)
                      .join(" · ")}
                  </div>
                )}
              </div>

              {isPending && (
                <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                  <Button
                    variant="outline"
                    size="sm"
                    aria-label="Approve"
                    disabled={approve.isPending || reject.isPending}
                    onClick={async () => {
                      try { await approve.mutateAsync(item.id); } catch { toast.error("Failed"); }
                    }}
                  >
                    <I.Check width={12} height={12} />
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    aria-label="Reject"
                    disabled={approve.isPending || reject.isPending}
                    onClick={async () => {
                      try { await reject.mutateAsync(item.id); } catch { toast.error("Failed"); }
                    }}
                  >
                    <I.X width={12} height={12} />
                  </Button>
                </div>
              )}
            </div>
          </div>
        );
      })}

      {canExecute && (
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <Button
            variant="primary"
            size="sm"
            disabled={isExecuting}
            loading={isExecuting}
            onClick={() => void handleExecute()}
          >
            <I.Check width={13} height={13} />
            Execute {approvedItems.length} approved action{approvedItems.length !== 1 ? "s" : ""}
          </Button>
        </div>
      )}
    </div>
  );
}

// ── Custom message components ─────────────────────────────────────────────────

function UserMessage() {
  return (
    <MessagePrimitive.Root className="copilot-msg-user">
      <div className="copilot-bubble-user">
        <MessagePrimitive.Parts>
          {({ part }) =>
            part.type === "text" ? (
              <span style={{ whiteSpace: "pre-wrap" }}>{part.text}</span>
            ) : null
          }
        </MessagePrimitive.Parts>
      </div>
      <MessageActions align="end" />
    </MessagePrimitive.Root>
  );
}

function MessageActions({ align = "start" }: { align?: "start" | "end" }) {
  return (
    <div className="copilot-msg-actions" data-align={align}>
      <ActionBarPrimitive.Root>
        <ActionBarPrimitive.Copy className="copilot-action-btn">Copy</ActionBarPrimitive.Copy>
        <ActionBarPrimitive.Edit className="copilot-action-btn">Edit</ActionBarPrimitive.Edit>
        <ActionBarPrimitive.Reload className="copilot-action-btn">Regenerate</ActionBarPrimitive.Reload>
      </ActionBarPrimitive.Root>
      <BranchPickerPrimitive.Root className="copilot-branch-picker">
        <BranchPickerPrimitive.Previous className="copilot-action-btn">Prev</BranchPickerPrimitive.Previous>
        <span><BranchPickerPrimitive.Number /> / <BranchPickerPrimitive.Count /></span>
        <BranchPickerPrimitive.Next className="copilot-action-btn">Next</BranchPickerPrimitive.Next>
      </BranchPickerPrimitive.Root>
    </div>
  );
}

function ReasoningGroup({ children }: { children: ReactNode }) {
  return (
    <details className="copilot-reasoning">
      <summary>Analysis path</summary>
      <div>{children}</div>
    </details>
  );
}

function ToolFallbackCard({ part }: { part: { toolName: string; args?: unknown; result?: unknown; isError?: boolean; status?: { type: string } } }) {
  return (
    <div className="copilot-tool-card" data-error={part.isError ? "true" : "false"}>
      <div className="copilot-tool-head">
        <span>{part.toolName.replaceAll("_", " ")}</span>
        <span className="copilot-tool-status">{part.status?.type ?? "complete"}</span>
      </div>
      <pre>{JSON.stringify(part.result ?? part.args ?? {}, null, 2)}</pre>
    </div>
  );
}

function SourcePill({ part }: { part: { title?: string; id: string } }) {
  return <span className="copilot-source-pill">{part.title ?? part.id}</span>;
}

function AssistantMessage({
  meta,
  onFollowUp,
}: {
  meta?: MessageMeta;
  onFollowUp: (q: string) => void;
}) {
  const message = useMessage();
  const timing = useMessageTiming();
  const isRunning = message.status?.type === "running";
  const isError = message.status?.type === "incomplete" && message.status.reason === "error";
  const plainText = message.content
    .filter((p): p is { type: "text"; text: string } => p.type === "text")
    .map((p) => p.text)
    .join("\n");

  return (
    <MessagePrimitive.Root className="copilot-msg-asst">
      <div className="copilot-avatar">
        <I.Brain width={14} height={14} style={{ color: "var(--accent)" }} />
      </div>

      <div style={{ flex: 1, minWidth: 0 }}>
        <div className="copilot-bubble-asst">
          {isError ? (
            <span style={{ whiteSpace: "pre-wrap" }}>{plainText}</span>
          ) : (
            <MessagePrimitive.GroupedParts
              groupBy={groupPartByType({
                reasoning: ["group-thought"],
                "tool-call": ["group-thought"],
                source: ["group-sources"],
              })}
              indicator="no-text"
            >
              {({ part, children }) => {
                switch (part.type) {
                  case "group-thought":
                    return <ReasoningGroup>{children}</ReasoningGroup>;
                  case "group-sources":
                    return <div className="copilot-sources">{children}</div>;
                  case "reasoning":
                    return <p className="copilot-reasoning-text">{part.text}</p>;
                  case "tool-call":
                    return part.toolUI ?? <ToolFallbackCard part={part} />;
                  case "source":
                    return <SourcePill part={part} />;
                  case "generative-ui":
                    return (
                      <MessagePrimitive.GenerativeUI
                        components={generativeUIComponents}
                        Fallback={({ component }) => (
                          <div className="copilot-gen-callout" data-tone="warning">
                            <strong>Could not render finance card</strong>
                            <p>Unknown component: {component}</p>
                          </div>
                        )}
                      />
                    );
                  case "text":
                    return isRunning ? (
                      <span style={{ whiteSpace: "pre-wrap" }}>{part.text}</span>
                    ) : (
                      <MarkdownTextPrimitive className="aui-md" />
                    );
                  case "indicator":
                    return <span className="copilot-cursor" aria-hidden="true" />;
                  default:
                    return null;
                }
              }}
            </MessagePrimitive.GroupedParts>
          )}
          {isRunning && <span className="copilot-cursor" aria-hidden="true" />}
        </div>

        {!isRunning && !isError && (
          <div className="copilot-msg-meta">
            {meta?.modelId && <span>{meta.providerId} · {meta.modelId}</span>}
            {(meta?.elapsedMs ?? timing?.totalStreamTime) && (
              <span>{Math.round((meta?.elapsedMs ?? timing?.totalStreamTime ?? 0) / 100) / 10}s</span>
            )}
            {typeof meta?.toolCount === "number" && <span>{meta.toolCount} tools</span>}
          </div>
        )}

        {/* Action bundle panel */}
        {!isRunning && !isError && meta?.bundleId && (
          <ActionBundlePanel bundleId={meta.bundleId} />
        )}

        {/* Follow-up suggestions */}
        {!isRunning && !isError && meta?.followUpQuestions && meta.followUpQuestions.length > 0 && (
          <div style={{ marginTop: 14 }}>
            <p className="eyebrow" style={{ marginBottom: 8, fontSize: 10.5 }}>
              Follow-up suggestions
            </p>
            <div className="row-sm wrap">
              {meta.followUpQuestions.map((q, i) => (
                <button
                  key={i}
                  className="chip"
                  onClick={() => onFollowUp(q)}
                  style={{ cursor: "pointer", fontSize: 11.5 }}
                >
                  <I.ArrowRight width={10} height={10} />
                  {q}
                </button>
              ))}
            </div>
          </div>
        )}
        <MessageActions />
      </div>
    </MessagePrimitive.Root>
  );
}

// ── EmptyThreadState ──────────────────────────────────────────────────────────

function EmptyThreadState({ onPrompt }: { onPrompt: (text: string) => void }) {
  return (
    <div className="copilot-empty">
      <div className="copilot-empty-icon">
        <I.Brain width={36} height={36} style={{ color: "var(--accent)" }} />
      </div>
      <h2 className="copilot-empty-title">What would you like to know?</h2>
      <p className="copilot-empty-sub">
        Ask anything about your finances — spending, goals, budget, or savings.
      </p>
      <div className="copilot-prompts-grid">
        {SUGGESTED_PROMPTS.map((p) => (
          <button
            key={p.label}
            className="copilot-prompt-card"
            onClick={() => onPrompt(p.label)}
          >
            <span className="copilot-prompt-icon">{p.icon}</span>
            <span>{p.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

// Wrapper that reads message id from context to look up metadata
function AssistantMessageWithMeta({
  metaByMessageId,
  latestMeta,
  onFollowUp,
}: {
  metaByMessageId: ReturnType<typeof useTauriCopilotRuntime>["metaByMessageId"];
  latestMeta: MessageMeta | null;
  onFollowUp: (q: string) => void;
}) {
  const message = useMessage();
  const custom = message.metadata?.custom as { messageId?: unknown; bundleId?: unknown } | undefined;
  const backendMessageId = typeof custom?.messageId === "string" ? custom.messageId : message.id;
  const msgMeta =
    metaByMessageId[backendMessageId] ??
    (typeof custom?.bundleId === "string" ? { bundleId: custom.bundleId } : undefined) ??
    latestMeta ??
    undefined;
  return <AssistantMessage meta={msgMeta} onFollowUp={onFollowUp} />;
}

// ── Error boundary ────────────────────────────────────────────────────────────

class ThreadErrorBoundary extends Component<
  { children: ReactNode },
  { error: string | null }
> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { error: null };
  }
  static getDerivedStateFromError(err: unknown): { error: string } {
    return { error: err instanceof Error ? err.message : String(err) };
  }
  componentDidCatch(err: Error, info: ErrorInfo) {
    console.error("[Copilot] Thread render error:", err, info);
  }
  render() {
    if (this.state.error) {
      return (
        <div className="copilot-error-state">
          <p style={{ fontWeight: 600, marginBottom: 8 }}>Something went wrong in the chat thread.</p>
          <pre style={{ fontSize: 11, color: "var(--ink-faint)", whiteSpace: "pre-wrap" }}>
            {this.state.error}
          </pre>
          <button
            className="btn"
            style={{ marginTop: 12 }}
            onClick={() => this.setState({ error: null })}
          >
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

// ── Thread with composer ──────────────────────────────────────────────────────

function CopilotThread({
  messages,
  metaByMessageId,
  latestMeta,
  onFollowUp,
}: {
  messages: ReturnType<typeof useTauriCopilotRuntime>["messages"];
  metaByMessageId: ReturnType<typeof useTauriCopilotRuntime>["metaByMessageId"];
  latestMeta: MessageMeta | null;
  onFollowUp: (q: string) => void;
}) {
  const threadRuntime = useThreadRuntime();
  const thread = useThread();
  const composerRef = useRef<HTMLTextAreaElement>(null);

  const handlePrompt = useCallback(
    (text: string) => {
      threadRuntime.composer.setText(text);
      setTimeout(() => composerRef.current?.focus(), 50);
    },
    [threadRuntime]
  );

  useEffect(() => {
    threadRuntime.reset(messages);
  }, [messages, threadRuntime]);

  return (
    <div className="copilot-thread-wrap">
      <ThreadPrimitive.Root className="copilot-thread">
        <ThreadPrimitive.Viewport className="copilot-viewport">
          <ThreadPrimitive.Empty>
            <EmptyThreadState onPrompt={handlePrompt} />
          </ThreadPrimitive.Empty>

          <ThreadPrimitive.Messages
          >
            {({ message }) =>
              message.role === "user" ? (
                <UserMessage />
              ) : (
                <AssistantMessageWithMeta
                  metaByMessageId={metaByMessageId}
                  latestMeta={latestMeta}
                  onFollowUp={onFollowUp}
                />
              )
            }
          </ThreadPrimitive.Messages>
        </ThreadPrimitive.Viewport>

        <div className="copilot-composer-wrap">
          <ComposerPrimitive.Root className="copilot-composer">
            <ComposerPrimitive.Input
              ref={composerRef}
              placeholder='Ask your financial analyst anything…'
              className="copilot-composer-input"
              autoFocus
            />
            {thread.isRunning ? (
              <ComposerPrimitive.Cancel className="copilot-send-btn" aria-label="Stop response">
                <I.X width={15} height={15} />
              </ComposerPrimitive.Cancel>
            ) : (
              <ComposerPrimitive.Send className="copilot-send-btn" aria-label="Send message">
                <I.Send width={15} height={15} />
              </ComposerPrimitive.Send>
            )}
          </ComposerPrimitive.Root>
          <p className="copilot-composer-hint muted">
            Press <kbd>↵</kbd> to send · <kbd>Shift+↵</kbd> for new line
          </p>
        </div>
      </ThreadPrimitive.Root>
    </div>
  );
}

function CopilotRuntimeProvider({
  runtime,
  children,
}: {
  runtime: ReturnType<typeof useTauriCopilotRuntime>["runtime"];
  children: ReactNode;
}) {
  const aui = useAui({ tools: Tools({ toolkit: copilotToolkit }) });
  return (
    <AssistantRuntimeProvider runtime={runtime} aui={aui}>
      {children}
    </AssistantRuntimeProvider>
  );
}

// ── Conversation sidebar ──────────────────────────────────────────────────────

function groupByDate(convs: ConversationSummary[]) {
  const now = new Date();
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const weekStart = todayStart - 6 * 24 * 60 * 60 * 1000;
  const today: ConversationSummary[] = [];
  const thisWeek: ConversationSummary[] = [];
  const earlier: ConversationSummary[] = [];
  for (const c of convs) {
    const t = new Date(c.updatedAt).getTime();
    if (t >= todayStart) today.push(c);
    else if (t >= weekStart) thisWeek.push(c);
    else earlier.push(c);
  }
  return { today, thisWeek, earlier };
}

function ConversationSidebar({
  activeId,
  onSelect,
  onNew,
}: {
  activeId: string | null;
  onSelect: (id: string) => void;
  onNew: () => void;
}) {
  const { data: convs = [] } = useConversations();
  const deleteConv = useDeleteConversation();
  const [search, setSearch] = useState("");
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const filtered = search
    ? convs.filter((c) => c.title.toLowerCase().includes(search.toLowerCase()))
    : convs;

  const { today, thisWeek, earlier } = groupByDate(filtered);

  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setDeletingId(id);
    try {
      await deleteConv.mutateAsync(id);
      toast.success("Conversation deleted");
    } catch {
      toast.error("Could not delete conversation");
    } finally {
      setDeletingId(null);
    }
  };

  const renderGroup = (label: string, items: ConversationSummary[]) => {
    if (items.length === 0) return null;
    return (
      <div key={label} style={{ marginBottom: 16 }}>
        <p className="eyebrow" style={{ padding: "0 12px", marginBottom: 4, fontSize: 10 }}>
          {label}
        </p>
        {items.map((c) => (
          <button
            key={c.id}
            className="copilot-thread-item"
            data-active={c.id === activeId}
            onClick={() => onSelect(c.id)}
            title={c.title}
          >
            <span className="copilot-thread-title">{c.title}</span>
            <span
              className="copilot-thread-delete"
              role="button"
              tabIndex={0}
              aria-label="Delete conversation"
              onClick={(e) => void handleDelete(c.id, e)}
            >
              {deletingId === c.id ? "…" : <I.X width={11} height={11} />}
            </span>
          </button>
        ))}
      </div>
    );
  };

  return (
    <aside className="copilot-sidebar">
      <div className="copilot-sidebar-header">
        <span className="eyebrow" style={{ fontSize: 10 }}>Conversations</span>
        <button className="copilot-new-btn" onClick={onNew} title="New conversation">
          <I.Plus width={14} height={14} />
        </button>
      </div>

      <div className="copilot-search">
        <I.Search width={13} height={13} />
        <input
          type="search"
          placeholder="Search…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="copilot-search-input"
        />
      </div>

      <div className="copilot-thread-list">
        {filtered.length === 0 ? (
          <div className="muted" style={{ padding: "12px", fontSize: 12.5 }}>
            {search ? "No matching conversations" : "No conversations yet."}
          </div>
        ) : (
          <>
            {renderGroup("Today", today)}
            {renderGroup("This week", thisWeek)}
            {renderGroup("Earlier", earlier)}
          </>
        )}
      </div>
    </aside>
  );
}

// ── Memory panel ──────────────────────────────────────────────────────────────

function MemoryPanel() {
  const { data: memories = [] } = useAgentMemory();
  const forget = useForgetAgentMemory();

  if (memories.length === 0) {
    return (
      <div className="copilot-memory-empty muted">
        <I.Brain width={28} height={28} style={{ marginBottom: 10, color: "var(--ink-faint)" }} />
        <p>No saved memory yet.</p>
      </div>
    );
  }

  return (
    <div className="stack stack-md" style={{ padding: "0 0 24px" }}>
      {memories.map((memory: AgentMemory) => (
        <div key={memory.id} className="card" style={{ padding: "14px 16px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start" }}>
            <div className="stack stack-xs">
              <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <Badge tone="accent">{memory.kind}</Badge>
                <span className="muted" style={{ fontSize: 11.5 }}>
                  {new Date(memory.createdAt).toLocaleDateString()}
                </span>
              </div>
              <div style={{ fontSize: 13.5 }}>{memory.description}</div>
              {memory.merchantKey && (
                <div className="muted" style={{ fontSize: 11.5 }}>Key: {memory.merchantKey}</div>
              )}
            </div>
            <Button
              variant="ghost"
              size="sm"
              loading={forget.isPending}
              onClick={async () => {
                try {
                  await forget.mutateAsync(memory.id);
                  toast.success("Forgot memory");
                } catch {
                  toast.error("Could not forget memory");
                }
              }}
            >
              Forget
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}

// ── Main screen ───────────────────────────────────────────────────────────────

export default function Copilot() {
  const createConv = useCreateConversation();
  const [activeConvId, setActiveConvId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"chat" | "memory">("chat");

  const { runtime, messages, latestMeta, metaByMessageId } = useTauriCopilotRuntime(activeConvId);

  const handleNew = useCallback(async () => {
    try {
      const id = await createConv.mutateAsync();
      setActiveConvId(id);
      setActiveTab("chat");
    } catch {
      toast.error("Could not create conversation");
    }
  }, [createConv]);

  const handleFollowUp = useCallback(
    (q: string) => {
      if (runtime.thread) {
        runtime.thread.composer.setText(q);
      }
    },
    [runtime]
  );

  // Pick up pre-filled prompt from CopilotNudge navigation
  useEffect(() => {
    const prefill = sessionStorage.getItem("copilot.prefill");
    if (prefill) {
      sessionStorage.removeItem("copilot.prefill");
      void createConv.mutateAsync().then((id) => {
        setActiveConvId(id);
        setTimeout(() => {
          if (runtime.thread) {
            runtime.thread.composer.setText(prefill);
          }
        }, 300);
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="copilot-screen">
      <ConversationSidebar
        activeId={activeConvId}
        onSelect={(id) => { setActiveConvId(id); setActiveTab("chat"); }}
        onNew={() => void handleNew()}
      />

      <div className="copilot-main">
        <header className="copilot-header">
          <div>
            <div className="eyebrow">
              <span
                className="dot"
                style={{ background: "var(--accent)", boxShadow: "0 0 6px var(--accent)" }}
              />
              COPILOT · AI FINANCIAL ANALYST
            </div>
            <h1 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Copilot</h1>
          </div>
          <div className="row-sm">
            <div className="toolbar" style={{ display: "inline-flex" }}>
              <button className={activeTab === "chat" ? "on" : ""} onClick={() => setActiveTab("chat")}>
                Chat
              </button>
              <button className={activeTab === "memory" ? "on" : ""} onClick={() => setActiveTab("memory")}>
                Memory
              </button>
            </div>
            <span className="chip" style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
              <span className="dot" aria-hidden="true" />
              AI-assisted
            </span>
          </div>
        </header>

        {activeTab === "chat" && (
          <>
            {activeConvId ? (
              <ThreadErrorBoundary>
                <CopilotRuntimeProvider runtime={runtime}>
                  <CopilotThread
                    messages={messages}
                    metaByMessageId={metaByMessageId}
                    latestMeta={latestMeta}
                    onFollowUp={handleFollowUp}
                  />
                </CopilotRuntimeProvider>
              </ThreadErrorBoundary>
            ) : (
              <div className="copilot-empty-screen">
                <div className="copilot-empty">
                  <div className="copilot-empty-icon">
                    <I.Brain width={44} height={44} style={{ color: "var(--accent)" }} />
                  </div>
                  <h2 className="copilot-empty-title">Your AI Financial Analyst</h2>
                  <p className="copilot-empty-sub">
                    Start a new conversation to get personalized advice and action plans
                    based on your real financial data.
                  </p>
                  <button
                    className="btn primary"
                    onClick={() => void handleNew()}
                    disabled={createConv.isPending}
                  >
                    <I.Plus width={14} height={14} />
                    Start a conversation
                  </button>
                  <div className="copilot-prompts-grid" style={{ marginTop: 32 }}>
                    {SUGGESTED_PROMPTS.map((p) => (
                      <button
                        key={p.label}
                        className="copilot-prompt-card"
                        onClick={() => {
                          void createConv.mutateAsync().then((id) => {
                            setActiveConvId(id);
                            setTimeout(() => {
                              if (runtime.thread) {
                                runtime.thread.composer.setText(p.label);
                              }
                            }, 300);
                          });
                        }}
                      >
                        <span className="copilot-prompt-icon">{p.icon}</span>
                        <span>{p.label}</span>
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </>
        )}

        {activeTab === "memory" && (
          <div className="main-inner">
            <MemoryPanel />
          </div>
        )}
      </div>
    </div>
  );
}
