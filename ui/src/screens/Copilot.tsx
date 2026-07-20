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
import { listen } from "@tauri-apps/api/event";
import { useQuery, useQueryClient } from "@tanstack/react-query";
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
import type { AssistantRuntime } from "@assistant-ui/react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeSanitize from "rehype-sanitize";
import "../styles/copilot-shell.css";
import * as I from "../components/Icons";
import Badge from "../components/Badge";
import Button from "../components/Button";
import {
  useApproveActionItem,
  useRejectActionItem,
  useActionBundle,
} from "../api/hooks/copilot";
import {
  useConversations,
  useDeleteConversation,
} from "../api/hooks/copilotChat";
import { useAccounts } from "../api/hooks/accounts";
import { useNavigate } from "react-router-dom";
import { commands, type AgentNavigationTarget } from "../api/client";
import { useTauriCopilotRuntime, type MessageMeta } from "../components/copilot/TauriRuntime";
import { sourcesFromToolTrace } from "../components/copilot/toolSources";
import { useTauriAgUiRuntime } from "../components/copilot/agUi/TauriAgUiRuntime";
import { isCopilotAgUiRuntimeEnabled } from "../components/copilot/agUi/featureFlag";
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
    recategorize_bulk: "Recategorize transactions",
    debt_payoff_plan: "Debt payoff plan",
    create_planned_transaction: "Planned transaction",
  };
  return labels[kind] ?? kind;
}

/**
 * Presentation-only transform: the backend's `reasoning` field is one joined
 * string (see ReasoningResult.reasoning in engine/mod.rs), not a structured
 * list. Splitting it into sentence-shaped steps lets the thinking block show
 * a numbered, connected list like the mockup without any backend change.
 */
export function splitReasoningIntoSteps(text: string): string[] {
  const trimmed = text.trim();
  if (!trimmed) return [];
  return trimmed
    .split(/(?<=[.!?])\s+(?=[A-Z])/)
    .map((s) => s.trim())
    .filter(Boolean);
}

// ── ActionBundlePanel ─────────────────────────────────────────────────────────

function ActionBundlePanel({ bundleId }: { bundleId: string }) {
  const { data: bundle } = useActionBundle(bundleId);
  const approve = useApproveActionItem();
  const reject = useRejectActionItem();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [isExecuting, setIsExecuting] = useState(false);
  const [executed, setExecuted] = useState(false);
  // Offered after a successful execution so the user can verify the change on
  // the screen it landed on instead of taking "done" on trust. Backend-derived
  // from the payloads that applied, so every path is a real screen.
  const [navTargets, setNavTargets] = useState<AgentNavigationTarget[]>([]);

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
      // Older servers omit this field; an absent offer is fine, a wrong one is not.
      setNavTargets(raw.navigation ?? []);
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
                    {(() => {
                      // Bulk actions carry a list payload (e.g. recategorize_bulk's
                      // `assignments`) — summarize the count instead of rendering
                      // "[object Object]". Scalar payloads render as key: value.
                      const scalars = Object.entries(payload).filter(
                        ([k, v]) =>
                          k !== "params" &&
                          v !== null &&
                          typeof v !== "object",
                      );
                      const lists = Object.entries(payload).filter(
                        ([, v]) => Array.isArray(v),
                      );
                      const parts = [
                        ...scalars.map(([k, v]) => `${k}: ${String(v)}`),
                        ...lists.map(([k, v]) => `${(v as unknown[]).length} ${k}`),
                      ];
                      return parts.join(" · ");
                    })()}
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

      {navTargets.length > 0 && (
        <div className="cp-followups" data-testid="copilot-action-navigation">
          <span className="cp-followups-lbl">See the change</span>
          <div className="cp-followups-row">
            {navTargets.map((target) => (
              <button key={target.path} className="cp-fu-chip" onClick={() => navigate(target.path)}>
                {target.label}
                <I.ArrowRight width={10} height={10} />
              </button>
            ))}
          </div>
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

/**
 * Collapsible "thinking" block shown above an assistant reply: a header with
 * running/done state, and a body with a "Plan", "Tool calls", and "Reasoning"
 * section — each its own `.cp-think-sec`.
 */
export function ThinkingBlock({ reasoningText, toolCalls, plan }: { reasoningText: string; toolCalls: ReactNode; plan?: string[] }) {
  const message = useMessage();
  const isRunning = message.status?.type === "running";
  const [open, setOpen] = useState(isRunning);
  const steps = splitReasoningIntoSteps(reasoningText);

  return (
    <div className={`cp-think ${isRunning ? "is-running" : "is-done"}`}>
      <button type="button" className="cp-think-hd" onClick={() => setOpen((o) => !o)}>
        <span className="cp-think-ico">
          {isRunning ? (
            <span className="cp-think-dots"><i /><i /><i /></span>
          ) : (
            <I.Check width={12} height={12} />
          )}
        </span>
        <span className="cp-think-title">
          {isRunning ? "Reasoning through your data…" : "Reasoned through your data"}
        </span>
        <I.Down className={`cp-think-chev ${open ? "open" : ""}`} width={14} height={14} />
      </button>
      {open && (
        <div className="cp-think-body">
          {plan && plan.length > 0 && (
            <div className="cp-think-sec">
              <p className="cp-think-sec-lbl">Plan</p>
              <div className="cp-think-plan">
                {plan.map((step, i) => (
                  <div key={i} className="cp-plan-item">
                    <span className="cp-plan-n">{i + 1}</span>
                    <span className="cp-plan-txt">{step}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
          <div className="cp-think-sec">
            <p className="cp-think-sec-lbl">Tool calls</p>
            <div className="cp-think-tools">{toolCalls}</div>
          </div>
          {steps.length > 0 && (
            <div className="cp-think-sec">
              <p className="cp-think-sec-lbl">Reasoning</p>
              <div className="cp-think-reason">
                {steps.map((step, i) => (
                  <div key={i} className="cp-reason-item">
                    <span className="cp-reason-n">{i + 1}</span>
                    <p className="cp-reason-txt">{step}</p>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function ToolFallbackCard({ part }: { part: { toolName: string; args?: unknown; result?: unknown; isError?: boolean; status?: { type: string } } }) {
  const [open, setOpen] = useState(false);
  const done = part.status?.type !== "running";
  // Collapse the detail panel the moment a tool finishes, so a row that was
  // expanded while still "running" doesn't carry that open state into "done".
  useEffect(() => {
    if (done) setOpen(false);
  }, [done]);
  const argsText = part.args && Object.keys(part.args as object).length > 0 ? JSON.stringify(part.args) : "";
  const resultSummary = (() => {
    const r = part.result as { summary?: string } | undefined;
    if (part.isError) return "error";
    if (r && typeof r.summary === "string") return r.summary;
    return done ? "done" : "running…";
  })();

  return (
    <div className={`cp-tool ${done ? "is-done" : "is-running"}`}>
      <button type="button" className="cp-tool-row" onClick={() => done && setOpen((o) => !o)}>
        <span className={`cp-tool-dot ${part.isError ? "is-error" : ""}`}>
          {done ? <I.Check width={10} height={10} /> : <span className="copilot-cursor" aria-hidden="true" />}
        </span>
        <span className="cp-tool-sig">
          <span className="cp-tool-fn">{part.toolName.replaceAll("_", " ")}</span>
          {argsText && <span className="cp-tool-args"> ({argsText})</span>}
        </span>
        <span className={`cp-tool-result ${part.isError ? "is-error" : ""}`}>{resultSummary}</span>
        {done && <I.Down className={`cp-tool-chev ${open ? "open" : ""}`} width={13} height={13} />}
      </button>
      {done && open && (
        <div className="cp-tool-detail">
          <pre className="cp-tool-pre">{JSON.stringify(part.result ?? part.args ?? {}, null, 2)}</pre>
        </div>
      )}
    </div>
  );
}

function SourcePill({ part }: { part: { title?: string; id: string } }) {
  return <span className="copilot-source-pill">{part.title ?? part.id}</span>;
}

/**
 * While an answer is still streaming, hide/complete the trailing markdown
 * constructs that would otherwise flash as raw markup before their closing
 * tokens arrive — an unclosed code fence (which would swallow the rest of the
 * prose) and an in-progress table whose `|---|` delimiter row hasn't streamed
 * yet (which renders as raw `| … |` pipes). Everything already complete keeps
 * rendering live; only the incomplete tail is held back for a beat until it
 * resolves. The final (done) render always uses the full, unmodified text.
 */
function stabilizeStreamingMarkdown(text: string): string {
  const fenceCount = text.split("\n").filter((l) => l.trimStart().startsWith("```")).length;
  if (fenceCount % 2 === 1) return `${text}\n\`\`\``;

  const lines = text.split("\n");
  let start = lines.length;
  while (start > 0 && (lines[start - 1] ?? "").trimStart().startsWith("|")) start--;
  if (start < lines.length) {
    const isDelimiter = (l: string) => {
      const t = l.trim();
      return t.includes("-") && /^[|\s:-]+$/.test(t);
    };
    const hasDelimiter = lines.slice(start).some(isDelimiter);
    if (!hasDelimiter) return lines.slice(0, start).join("\n");
  }
  return text;
}

function CopilotMarkdown({ text, streaming = false }: { text: string; streaming?: boolean }) {
  const markdown = streaming ? stabilizeStreamingMarkdown(text) : text;
  return (
    <div className="agent-rich-markdown copilot-answer-md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeSanitize]}
        components={{
          table: ({ children }) => (
            <div className="agent-rich-table-wrap">
              <table className="tbl">{children}</table>
            </div>
          ),
          a: ({ children, href }) => (
            <a href={href} target="_blank" rel="noreferrer">
              {children}
            </a>
          ),
        }}
      >
        {markdown}
      </ReactMarkdown>
    </div>
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
  const navigate = useNavigate();
  const isRunning = message.status?.type === "running";
  const isError = message.status?.type === "incomplete" && message.status.reason === "error";
  const plainText = message.content
    .filter((p): p is { type: "text"; text: string } => p.type === "text")
    .map((p) => p.text)
    .join("\n");
  const reasoningText = message.content
    .filter((p): p is { type: "reasoning"; text: string } => p.type === "reasoning")
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
        <div className="cp-turn-hd">
          <div className={`cp-agent-mark ${isRunning ? "is-thinking" : ""}`}>
            <span className="cp-agent-core" />
          </div>
          <span className="cp-turn-name">Copilot</span>
          {meta?.modelId && <span className="cp-turn-model">{meta.providerId} · {meta.modelId}</span>}
          <div className="cp-src-rail">
            {sourcesFromToolTrace(meta?.toolTrace).map((label) => (
              <span key={label} className="cp-src is-on">{label}</span>
            ))}
          </div>
        </div>
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
                    return <ThinkingBlock reasoningText={reasoningText} toolCalls={children} plan={meta?.plan} />;
                  case "group-sources":
                    return <div className="copilot-sources">{children}</div>;
                  case "reasoning":
                    // Reasoning content is rendered via ThinkingBlock's own
                    // "Reasoning" section (sourced from reasoningText above),
                    // so it isn't also rendered inline here — that would show
                    // it twice, once as a stray paragraph mixed into the
                    // "Tool calls" children and once as numbered steps.
                    return null;
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
                    return <CopilotMarkdown text={part.text} streaming={isRunning} />;
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
          <div className="cp-turn-ft">
            <span className="copilot-grounded" title="Answers are computed from your local FinSight data only.">
              <I.Lock width={10} height={10} />
              Grounded on your data
            </span>
            {meta?.modelId && <span>{meta.providerId} · {meta.modelId}</span>}
            {(meta?.elapsedMs ?? timing?.totalStreamTime) && (
              <span>{Math.round((meta?.elapsedMs ?? timing?.totalStreamTime ?? 0) / 100) / 10}s</span>
            )}
            {typeof meta?.toolCount === "number" && <span>{meta.toolCount} tools</span>}
            {typeof meta?.cachedTokens === "number" && meta.cachedTokens > 0 && (
              <span
                title={`${meta.cachedTokens.toLocaleString()} of ${(meta.promptTokens ?? 0).toLocaleString()} prompt tokens served from the provider's cache`}
              >
                {meta.promptTokens && meta.promptTokens > 0
                  ? `${Math.round((meta.cachedTokens / meta.promptTokens) * 100)}% cached`
                  : `${meta.cachedTokens.toLocaleString()} cached`}
              </span>
            )}
          </div>
        )}

        {/* Action bundle panel */}
        {!isRunning && !isError && meta?.bundleId && (
          <ActionBundlePanel bundleId={meta.bundleId} />
        )}

        {/* Missing data — what the answer could not find, and where to add it.
            The Copilot deliberately withholds confident debt advice when APR
            or minimum-payment data is absent; without a way to unblock it,
            that reads as unhelpful rather than careful. */}
        {!isRunning && !isError && meta?.missingData && meta.missingData.length > 0 && (
          <div className="cp-missing" data-testid="copilot-missing-data">
            <span className="cp-missing-lbl">To answer this more precisely</span>
            <ul className="cp-missing-list">
              {meta.missingData.map((item, i) => (
                <li key={i}>
                  <span>{item.message}</span>
                  {item.actionLabel && item.actionPath && (
                    <button
                      className="cp-missing-cta"
                      onClick={() => navigate(item.actionPath!)}
                    >
                      {item.actionLabel}
                      <I.ArrowRight width={10} height={10} />
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* Follow-up suggestions */}
        {!isRunning && !isError && meta?.followUpQuestions && meta.followUpQuestions.length > 0 && (
          <div className="cp-followups">
            <span className="cp-followups-lbl">Ask next</span>
            <div className="cp-followups-row">
              {meta.followUpQuestions.map((q, i) => (
                <button
                  key={i}
                  className="cp-fu-chip"
                  onClick={() => onFollowUp(q)}
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

/**
 * Grounding stats shown under the Copilot hero. Uses REAL local counts (never
 * fabricated) so the empty state is honest: when no data has been imported it
 * says so plainly, which is also the after-Delete-All-Data experience.
 */
function CopilotGroundingStats() {
  const { data: accounts = [] } = useAccounts();
  const { data: txnCount = 0 } = useQuery({
    queryKey: ["transaction-count"],
    queryFn: async () => {
      const res = await commands.getTransactionCount();
      return res.status === "ok" ? res.data : 0;
    },
  });

  if (txnCount === 0 && accounts.length === 0) {
    return (
      <p className="copilot-empty-ground copilot-empty-ground-empty">
        <I.Lock width={11} height={11} />
        No financial data imported yet — import a CSV to give the Copilot something to work with.
      </p>
    );
  }

  return (
    <div className="copilot-empty-ground">
      <span>
        <I.Flow width={11} height={11} />
        {txnCount.toLocaleString()} transaction{txnCount === 1 ? "" : "s"}
      </span>
      <span>
        <I.Wallet width={11} height={11} />
        {accounts.length} account{accounts.length === 1 ? "" : "s"}
      </span>
      <span>
        <I.Lock width={11} height={11} />
        100% local
      </span>
    </div>
  );
}

function EmptyThreadState({
  onPrompt,
  children,
}: {
  onPrompt: (text: string) => void;
  children: ReactNode;
}) {
  const h = new Date().getHours();
  const greeting = h < 12 ? "Good morning" : h < 17 ? "Good afternoon" : "Good evening";

  return (
    <div className="cp-hero">
      <div className="cp-hero-glow" aria-hidden="true">
        <div className="cp-glow-orb cp-glow-1" />
        <div className="cp-glow-orb cp-glow-2" />
      </div>
      <div className="cp-hero-inner">
        <div className="cp-hero-avatar">
          <span className="cp-avatar-ring">
            <span className="cp-avatar-core" />
          </span>
        </div>
        <h1 className="cp-hero-h1">{greeting}.</h1>
        <p className="cp-hero-sub">
          Ask for a plan, explanation, cleanup pass, or tradeoff analysis. FinSight can use
          your local accounts, budgets, goals, and transactions when a tool is needed.
        </p>
        {children}
        <div className="cp-hero-chips">
          {SUGGESTED_PROMPTS.map((p) => (
            <button
              key={p.label}
              type="button"
              className="cp-hero-chip"
              onClick={() => onPrompt(p.label)}
              title={p.detail}
            >
              <I.Sparkle width={12} height={12} className="cp-chip-ico" />
              <span>{p.label}</span>
            </button>
          ))}
        </div>
        <CopilotGroundingStats />
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
                <CopilotComposerBox composerRef={composerRef} isRunning={thread.isRunning} latestMeta={latestMeta} />
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
                <CopilotComposerBox composerRef={composerRef} isRunning={thread.isRunning} latestMeta={latestMeta} />
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
  latestMeta,
}: {
  composerRef: React.RefObject<HTMLTextAreaElement>;
  isRunning: boolean;
  latestMeta: MessageMeta | null;
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
      <div className="cp-composer-model">
        <span className="cp-model-dot" />
        <span>{latestMeta?.modelId ? `${latestMeta.providerId} · ${latestMeta.modelId}` : "Copilot ready"}</span>
      </div>
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

function CopilotHeader({ threadControls = true }: { threadControls?: boolean }) {
  const [historyOpen, setHistoryOpen] = useState(false);

  return (
    <header className="copilot-header">
      <div className="copilot-title-block">
        <p className="copilot-kicker">Workshop</p>
        <h1>Copilot</h1>
        <span>Plan, explain, and act on your FinSight data.</span>
      </div>
      {threadControls && (
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
      )}
    </header>
  );
}

function CopilotAgUiHeader({
  activeConversationId,
  onSelectConversation,
  onNewThread,
}: {
  activeConversationId: string | null;
  onSelectConversation: (conversationId: string) => void;
  onNewThread: () => void;
}) {
  const [historyOpen, setHistoryOpen] = useState(false);
  const { data: conversations = [] } = useConversations();
  const deleteConversation = useDeleteConversation();

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
              <div className="copilot-history-list">
                {conversations.length === 0 ? (
                  <div className="copilot-history-empty">No conversations yet.</div>
                ) : (
                  conversations.map((conversation) => (
                    <div
                      key={conversation.id}
                      className="copilot-history-row"
                      data-active={conversation.id === activeConversationId ? "true" : "false"}
                    >
                      <button
                        type="button"
                        className="copilot-history-trigger"
                        onClick={() => {
                          onSelectConversation(conversation.id);
                          setHistoryOpen(false);
                        }}
                      >
                        <span>{conversation.title || "New conversation"}</span>
                      </button>
                      <button
                        type="button"
                        className="copilot-history-delete"
                        aria-label="Delete thread"
                        disabled={deleteConversation.isPending}
                        onClick={async () => {
                          try {
                            await deleteConversation.mutateAsync(conversation.id);
                            if (conversation.id === activeConversationId) onNewThread();
                          } catch {
                            toast.error("Could not delete conversation");
                          }
                        }}
                      >
                        <I.Trash width={13} height={13} />
                      </button>
                    </div>
                  ))
                )}
              </div>
            </div>
          )}
        </div>
        <button
          type="button"
          className="copilot-top-button copilot-top-button-primary"
          title="New thread"
          onClick={onNewThread}
        >
          <I.Plus width={15} height={15} />
          New thread
        </button>
      </div>
    </header>
  );
}

function CopilotRuntimeProvider({
  runtime,
  children,
}: {
  runtime: AssistantRuntime;
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
  if (isCopilotAgUiRuntimeEnabled()) {
    return <CopilotAgUiEnabled />;
  }

  return <CopilotLocalRuntime />;
}

function CopilotLocalRuntime() {
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

function CopilotAgUiEnabled() {
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);
  // Bumped to force the session (and its AG-UI history adapter) to remount and
  // reload, so a background "deep answer" persisted to the active conversation
  // shows up without the user re-opening it.
  const [reloadNonce, setReloadNonce] = useState(0);
  const activeIdRef = useRef(activeConversationId);
  activeIdRef.current = activeConversationId;

  // A heavy question's fuller follow-up arrives asynchronously (background mode).
  // The message is already persisted; surface it — reload the active thread, or
  // toast to open the conversation it landed in.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<{ conversationId?: string }>("copilot-async-answer", (event) => {
      const convId = event.payload?.conversationId;
      if (!convId) return;
      if (convId === activeIdRef.current) {
        setReloadNonce((n) => n + 1);
        toast.success("Your fuller analysis is ready", {
          description: "Added to this conversation.",
        });
      } else {
        toast.info("Your fuller analysis is ready", {
          description: "Open the conversation to read it.",
          action: { label: "View", onClick: () => setActiveConversationId(convId) },
        });
      }
    }).then((u) => {
      unlisten = u;
    });
    return () => unlisten?.();
  }, []);

  return (
    <CopilotAgUiSession
      key={`${activeConversationId ?? "new"}-${reloadNonce}`}
      activeConversationId={activeConversationId}
      onSelectConversation={setActiveConversationId}
      onNewThread={() => setActiveConversationId(null)}
    />
  );
}

function CopilotAgUiSession({
  activeConversationId,
  onSelectConversation,
  onNewThread,
}: {
  activeConversationId: string | null;
  onSelectConversation: (conversationId: string) => void;
  onNewThread: () => void;
}) {
  const { runtime, latestMeta, metaByMessageId } = useTauriAgUiRuntime(activeConversationId);

  return (
    <CopilotRuntimeProvider runtime={runtime}>
      <div className="copilot-screen copilot-finsight-chat" data-runtime="ag-ui">
        <CopilotAgUiHeader
          activeConversationId={activeConversationId}
          onSelectConversation={onSelectConversation}
          onNewThread={onNewThread}
        />
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
