/**
 * Copilot screen — full ChatGPT-style threaded AI chat.
 *
 * Architecture:
 *   • Base assistant-ui demo-style single thread shell
 *   • @assistant-ui/react Thread driven by TauriRuntime + copilot-stream-frame
 *   • Structured parts for text, reasoning, tool calls, sources, and generative UI
 *   • Action-item approval preserved inline below assistant bubbles as compatibility UI
 */
import { useState, useEffect, useRef, useCallback, Component } from "react";
import type { ReactNode, ErrorInfo } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  AssistantRuntimeProvider,
  ThreadPrimitive,
  ThreadListPrimitive,
  ThreadListItemPrimitive,
  MessagePrimitive,
  ComposerPrimitive,
  ActionBarPrimitive,
  BranchPickerPrimitive,
  AuiIf,
  ErrorPrimitive,
  Tools,
  groupPartByType,
  useMessage,
  useMessageTiming,
  useAui,
  useThreadRuntime,
  useThread,
} from "@assistant-ui/react";
import { StreamdownTextPrimitive } from "@assistant-ui/react-streamdown";
import { code } from "@streamdown/code";
import { cjk } from "@streamdown/cjk";
import { math } from "@streamdown/math";
import { mermaid } from "@streamdown/mermaid";
import "katex/dist/katex.min.css";
import "streamdown/styles.css";
import * as I from "../components/Icons";
import Badge from "../components/Badge";
import Button from "../components/Button";
import {
  useApproveActionItem,
  useRejectActionItem,
  useActionBundle,
} from "../api/hooks/copilot";
import { useTauriCopilotRuntime, type MessageMeta } from "../components/copilot/TauriRuntime";
import {
  copilotToolkit,
  generativeUIComponents,
} from "../components/copilot/renderers";
import { invoke } from "@tauri-apps/api/core";
import type { ExecutionSummary } from "../api/client";

// ── Constants ────────────────────────────────────────────────────────────────

const SUGGESTED_PROMPTS = [
  {
    label: "Plan next month's budget",
    detail: "Use recent spending and goals to propose a practical allocation.",
  },
  {
    label: "Clean up uncategorized transactions",
    detail: "Find messy transactions and suggest safe categorization steps.",
  },
  {
    label: "Improve my savings rate",
    detail: "Identify cuts that matter without making the budget brittle.",
  },
  {
    label: "Explain this month's spending",
    detail: "Summarize the drivers, outliers, and trend changes.",
  },
  {
    label: "Check my financial risks",
    detail: "Review cash buffer, debt pressure, and upcoming obligations.",
  },
  {
    label: "Create a faster debt payoff plan",
    detail: "Compare payoff ordering and monthly contribution tradeoffs.",
  },
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
        <ActionBarPrimitive.FeedbackPositive className="copilot-action-btn">Helpful</ActionBarPrimitive.FeedbackPositive>
        <ActionBarPrimitive.FeedbackNegative className="copilot-action-btn">Not helpful</ActionBarPrimitive.FeedbackNegative>
        <ActionBarPrimitive.Reload className="copilot-action-btn">Regenerate</ActionBarPrimitive.Reload>
      </ActionBarPrimitive.Root>
      <AuiIf condition={({ message }) => message.branchCount > 1}>
        <BranchPickerPrimitive.Root className="copilot-branch-picker">
          <BranchPickerPrimitive.Previous className="copilot-action-btn">Prev</BranchPickerPrimitive.Previous>
          <span><BranchPickerPrimitive.Number /> / <BranchPickerPrimitive.Count /></span>
          <BranchPickerPrimitive.Next className="copilot-action-btn">Next</BranchPickerPrimitive.Next>
        </BranchPickerPrimitive.Root>
      </AuiIf>
    </div>
  );
}

function ReasoningGroup({ children }: { children: ReactNode }) {
  const message = useMessage();
  const isRunning = message.status?.type === "running";

  return (
    <details className="copilot-reasoning" open={isRunning}>
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

function CopilotMarkdown() {
  return (
    <StreamdownTextPrimitive
      className="aui-md"
      containerClassName="copilot-streamdown"
      plugins={{ code, cjk, math, mermaid }}
      shikiTheme={["github-dark", "github-dark"]}
      security={{
        allowedImagePrefixes: ["https://", "data:image/"],
        allowedLinkPrefixes: ["https://", "http://", "mailto:"],
        allowedProtocols: ["https", "http", "mailto"],
        allowDataImages: true,
      }}
    />
  );
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
  const hasReadableContent = message.content.some((part) => {
    if (part.type === "text" || part.type === "reasoning") {
      return Boolean(part.text.trim());
    }
    return part.type === "tool-call" || part.type === "generative-ui" || part.type === "source";
  });

  return (
    <MessagePrimitive.Root className="copilot-msg-asst">
      <div className="copilot-avatar">
        <I.Brain width={14} height={14} style={{ color: "var(--accent)" }} />
      </div>

      <div style={{ flex: 1, minWidth: 0 }}>
        <MessagePrimitive.Quote>
          {({ text }) => (
            <blockquote className="copilot-quote">
              {text}
            </blockquote>
          )}
        </MessagePrimitive.Quote>
        <div className="copilot-bubble-asst">
          {isRunning && !hasReadableContent && (
            <div className="copilot-progress">Analyzing your local financial data…</div>
          )}
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
                      <CopilotMarkdown />
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
        <MessagePrimitive.Error>
          <ErrorPrimitive.Root className="copilot-message-error">
            <ErrorPrimitive.Message />
          </ErrorPrimitive.Root>
        </MessagePrimitive.Error>

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

function EmptyThreadState({
  onPrompt,
  children,
}: {
  onPrompt: (text: string) => void;
  children: ReactNode;
}) {
  return (
    <div className="copilot-empty">
      <p className="copilot-empty-kicker">Private financial assistant</p>
      <h2 className="copilot-empty-title">What should we work through?</h2>
      <p className="copilot-empty-sub">
        Ask for a plan, explanation, cleanup pass, or tradeoff analysis. FinSight can use
        your local accounts, budgets, goals, and transactions when a tool is needed.
      </p>
      {children}
      <div className="copilot-prompts-grid">
        {SUGGESTED_PROMPTS.map((p) => (
          <button
            key={p.label}
            className="copilot-prompt-card"
            onClick={() => onPrompt(p.label)}
          >
            <span className="copilot-prompt-mark" aria-hidden="true" />
            <span className="copilot-prompt-copy">
              <strong>{p.label}</strong>
              <small>{p.detail}</small>
            </span>
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
  metaByMessageId,
  latestMeta,
}: {
  metaByMessageId: ReturnType<typeof useTauriCopilotRuntime>["metaByMessageId"];
  latestMeta: MessageMeta | null;
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

  return (
    <div className="copilot-thread-wrap">
      <ThreadPrimitive.Root className="copilot-thread">
        <ThreadPrimitive.Viewport className="copilot-viewport copilot-scrollbar">
          <AuiIf condition={(s) => s.thread.isEmpty}>
            <EmptyThreadState onPrompt={handlePrompt}>
              <div className="copilot-empty-composer">
                <CopilotComposerBox composerRef={composerRef} isRunning={thread.isRunning} />
              </div>
            </EmptyThreadState>
          </AuiIf>

          <ThreadPrimitive.Messages
          >
            {({ message }) =>
              message.role === "user" ? (
                <UserMessage />
              ) : (
                  <AssistantMessageWithMeta
                    metaByMessageId={metaByMessageId}
                    latestMeta={latestMeta}
                    onFollowUp={handlePrompt}
                  />
                )
            }
          </ThreadPrimitive.Messages>
          <AuiIf condition={(s) => !s.thread.isEmpty}>
            <ThreadPrimitive.ScrollToBottom className="copilot-scroll-bottom" aria-label="Scroll to bottom">
              <I.ArrowDown width={16} height={16} />
            </ThreadPrimitive.ScrollToBottom>
          </AuiIf>
          <AuiIf condition={(s) => !s.thread.isEmpty}>
            <ThreadPrimitive.ViewportFooter className="copilot-viewport-footer">
              <div className="copilot-composer-wrap">
                <CopilotComposerBox composerRef={composerRef} isRunning={thread.isRunning} />
              </div>
            </ThreadPrimitive.ViewportFooter>
          </AuiIf>
          </ThreadPrimitive.Viewport>
      </ThreadPrimitive.Root>
    </div>
  );
}

function CopilotComposerBox({
  composerRef,
  isRunning,
}: {
  composerRef: React.RefObject<HTMLTextAreaElement>;
  isRunning: boolean;
}) {
  return (
    <ComposerPrimitive.Root className="copilot-composer">
      <button
        type="button"
        className="copilot-context-btn"
        title="FinSight automatically attaches relevant financial context"
        onClick={() => composerRef.current?.focus()}
      >
        <I.Plus width={16} height={16} />
      </button>
      <ComposerPrimitive.Input
        ref={composerRef}
        placeholder='Ask FinSight to plan, explain, or clean up your finances...'
        className="copilot-composer-input"
        autoFocus
      />
      {isRunning ? (
        <ComposerPrimitive.Cancel className="copilot-send-btn" aria-label="Stop response">
          <I.X width={15} height={15} />
        </ComposerPrimitive.Cancel>
      ) : (
        <ComposerPrimitive.Send className="copilot-send-btn" aria-label="Send message">
          <I.ArrowUp width={18} height={18} />
        </ComposerPrimitive.Send>
      )}
    </ComposerPrimitive.Root>
  );
}

function CopilotHeader() {
  const [historyOpen, setHistoryOpen] = useState(false);

  return (
    <header className="copilot-header">
      <div className="copilot-title-block">
        <p className="copilot-kicker">Workshop</p>
        <h1>Copilot</h1>
        <span>Plan, explain, and act on your FinSight data.</span>
      </div>
      <div className="copilot-header-actions">
        <div className="copilot-history-menu">
          <button
            type="button"
            className="copilot-top-button"
            aria-expanded={historyOpen}
            onClick={() => setHistoryOpen((open) => !open)}
          >
            <I.Today width={15} height={15} />
            History
            <I.Down width={13} height={13} />
          </button>
          {historyOpen && (
            <div className="copilot-history-popover" role="menu">
              <p>Recent threads</p>
              <ThreadListPrimitive.Root className="copilot-history-list">
                <ThreadListPrimitive.Items>
                  {({ threadListItem }) => (
                    <ThreadListItemPrimitive.Root className="copilot-history-row">
                      <ThreadListItemPrimitive.Trigger
                        className="copilot-history-trigger"
                        onClick={() => setHistoryOpen(false)}
                      >
                        <ThreadListItemPrimitive.Title fallback={threadListItem.title || "New conversation"} />
                      </ThreadListItemPrimitive.Trigger>
                      <ThreadListItemPrimitive.Delete
                        className="copilot-history-delete"
                        aria-label="Delete thread"
                      >
                        <I.Trash width={13} height={13} />
                      </ThreadListItemPrimitive.Delete>
                    </ThreadListItemPrimitive.Root>
                  )}
                </ThreadListPrimitive.Items>
              </ThreadListPrimitive.Root>
            </div>
          )}
        </div>
        <ThreadListPrimitive.New className="copilot-top-button copilot-top-button-primary" title="New thread">
          <I.Plus width={15} height={15} />
          New thread
        </ThreadListPrimitive.New>
      </div>
    </header>
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

function CopilotPrefill() {
  const aui = useAui();
  const threadRuntime = useThreadRuntime();

  useEffect(() => {
    const prefill = sessionStorage.getItem("copilot.prefill");
    if (!prefill) return;
    sessionStorage.removeItem("copilot.prefill");
    void aui.threads().switchToNewThread();
    window.setTimeout(() => {
      threadRuntime.composer.setText(prefill);
    }, 100);
  }, [aui, threadRuntime]);

  return null;
}

// ── Main screen ───────────────────────────────────────────────────────────────

export default function Copilot() {
  const { runtime, latestMeta, metaByMessageId } = useTauriCopilotRuntime();

  return (
    <CopilotRuntimeProvider runtime={runtime}>
      <div className="copilot-screen copilot-finsight-chat">
        <CopilotHeader />
        <ThreadErrorBoundary>
          <CopilotPrefill />
          <CopilotThread
            metaByMessageId={metaByMessageId}
            latestMeta={latestMeta}
          />
        </ThreadErrorBoundary>
      </div>
    </CopilotRuntimeProvider>
  );
}
