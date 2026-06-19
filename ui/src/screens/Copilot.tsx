import { useState, useRef, useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import * as I from "../components/Icons";
import Button from "../components/Button";
import Card from "../components/Card";
import Badge from "../components/Badge";
import EmptyState from "../components/EmptyState";
import {
  useActionBundles,
  useActionBundle,
  useApproveActionItem,
  useRejectActionItem,
  useExecutionLog,
} from "../api/hooks/copilot";
import type { AgentActionBundle, AgentActionItem, AppError } from "../api/client";

// ── Local types until bindings are regenerated with Phase 3-4 backend ──────

interface CopilotPlanResult {
  bundleId: string;
  answer: string;
  assumptions: string[];
  followUpQuestions: string[];
  forecastSummary: string | null;
}

interface ExecutionItemResult {
  itemId: string;
  actionKind: string;
  status: string;
  summary: string | null;
  error: string | null;
}

interface ExecutionSummary {
  bundleId: string;
  succeeded: number;
  failed: number;
  results: ExecutionItemResult[];
}

// ── Helpers ─────────────────────────────────────────────────────────────────

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

function actionKindIcon(kind: string): React.ReactNode {
  switch (kind) {
    case "set_budget": return <I.Lego width={13} height={13} />;
    case "update_goal_monthly":
    case "update_goal_target": return <I.Goal width={13} height={13} />;
    case "set_transaction_category":
    case "set_transaction_flag": return <I.Tag width={13} height={13} />;
    case "create_rule": return <I.Bolt width={13} height={13} />;
    case "save_scenario": return <I.Spark width={13} height={13} />;
    case "generate_report": return <I.Flow width={13} height={13} />;
    default: return <I.Cpu width={13} height={13} />;
  }
}

function ConfidenceBadge({ c }: { c: number }) {
  const pct = Math.round(c * 100);
  const tone = c >= 0.8 ? "positive" : c >= 0.6 ? "warning" : "negative";
  return <Badge tone={tone}>{pct}% confident</Badge>;
}

function ItemStatusBadge({ status }: { status: string }) {
  switch (status) {
    case "approved":  return <Badge tone="positive" dot>Approved</Badge>;
    case "rejected":  return <Badge tone="negative" dot>Rejected</Badge>;
    case "executed":  return <Badge tone="positive">Executed</Badge>;
    case "failed":    return <Badge tone="negative">Failed</Badge>;
    default:          return <Badge>Pending review</Badge>;
  }
}

// ── Suggested prompts ───────────────────────────────────────────────────────

const SUGGESTED_PROMPTS = [
  "Plan next month's budget",
  "How much should I save toward each goal?",
  "What can I cut to improve my savings rate?",
  "Explain my spending this month",
  "Clean up uncategorized transactions",
  "What financial risks am I facing?",
  "Can I afford a $2,000 expense right now?",
  "Create a plan to pay off debt faster",
];

// ── Action item row ─────────────────────────────────────────────────────────

function ActionItemRow({
  item,
  selected,
  onToggle,
  disabled,
}: {
  item: AgentActionItem;
  selected: boolean;
  onToggle: (id: string) => void;
  disabled: boolean;
}) {
  const approve = useApproveActionItem();
  const reject = useRejectActionItem();
  const isPendingReview = item.status === "pending";

  let payload: Record<string, unknown> = {};
  try { payload = JSON.parse(item.payloadJson) as Record<string, unknown>; } catch { /* ok */ }

  return (
    <div
      className={`card copilot-action-item${selected ? " selected" : ""}${
        item.status === "rejected" ? " rejected" : ""
      }`}
      style={{
        padding: "12px 16px",
        marginBottom: 6,
        background: selected ? "var(--accent-2)" : "var(--surface-2)",
        borderColor: selected ? "var(--accent-3)" : "var(--line)",
        opacity: item.status === "rejected" ? 0.5 : 1,
      }}
      role="listitem"
      aria-selected={selected}
    >
      <div className="row-md" style={{ alignItems: "flex-start" }}>
        {isPendingReview ? (
          <label className="row-xs" style={{ marginTop: 2, cursor: "pointer", flexShrink: 0 }}>
            <input
              type="checkbox"
              checked={selected}
              disabled={disabled}
              onChange={() => onToggle(item.id)}
              aria-label={`Select action: ${actionKindLabel(item.actionKind)}`}
            />
          </label>
        ) : (
          <div className="row" style={{ width: 16, height: 16, marginTop: 2, flexShrink: 0, justifyContent: "center" }}>
            {item.status === "approved" || item.status === "executed" ? (
              <I.Check width={14} height={14} style={{ color: "var(--positive)" }} />
            ) : item.status === "rejected" ? (
              <I.X width={14} height={14} style={{ color: "var(--negative)" }} />
            ) : null}
          </div>
        )}

        <div className="grow stack stack-xs" style={{ minWidth: 0 }}>
          <div className="row-sm wrap" style={{ alignItems: "center" }}>
            <span className="muted" aria-hidden="true">
              {actionKindIcon(item.actionKind)}
            </span>
            <span style={{ fontSize: 13.5, fontWeight: 600 }}>
              {actionKindLabel(item.actionKind)}
            </span>
            <ConfidenceBadge c={item.confidence} />
            {!isPendingReview && <ItemStatusBadge status={item.status} />}
          </div>

          <p className="muted" style={{ margin: 0, fontSize: 12.5, lineHeight: 1.5 }}>
            {item.rationale}
          </p>

          {Object.keys(payload).length > 0 && (
            <div className="num" style={{
              marginTop: 6,
              padding: "4px 8px",
              background: "var(--elevated)",
              borderRadius: 5,
              fontSize: 11.5,
              fontFamily: "var(--mono)",
              color: "var(--ink-faint)",
              wordBreak: "break-all",
            }}>
              {Object.entries(payload)
                .filter(([k]) => !["params"].includes(k))
                .map(([k, v]) => `${k}: ${String(v)}`)
                .join(" · ")}
            </div>
          )}
        </div>

        {isPendingReview && (
          <div className="row-xs" style={{ flexShrink: 0 }}>
            <Button
              variant="outline"
              size="sm"
              aria-label="Approve this action"
              title="Approve this action"
              disabled={approve.isPending || reject.isPending}
              loading={approve.isPending}
              onClick={async () => {
                try { await approve.mutateAsync(item.id); } catch { toast.error("Failed to approve"); }
              }}
            >
              <I.Check width={12} height={12} />
            </Button>
            <Button
              variant="outline"
              size="sm"
              aria-label="Reject this action"
              title="Reject this action"
              disabled={approve.isPending || reject.isPending}
              loading={reject.isPending}
              onClick={async () => {
                try { await reject.mutateAsync(item.id); } catch { toast.error("Failed to reject"); }
              }}
            >
              <I.X width={12} height={12} />
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Plan result card ────────────────────────────────────────────────────────

function PlanCard({
  planResult,
  bundle,
  onFollowUp,
}: {
  planResult: CopilotPlanResult;
  bundle: AgentActionBundle | null | undefined;
  onFollowUp: (q: string) => void;
}) {
  const qc = useQueryClient();
  const [selectedItems, setSelectedItems] = useState<Set<string>>(() => new Set());
  const [executionResult, setExecutionResult] = useState<ExecutionSummary | null>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const { data: execLog } = useExecutionLog(bundle?.id ?? null);

  // Pre-select all pending items
  useEffect(() => {
    if (bundle?.items) {
      const pendingIds = bundle.items
        .filter((i) => i.status === "pending")
        .map((i) => i.id);
      setSelectedItems(new Set(pendingIds));
    }
  }, [bundle?.id]);

  const toggleItem = (id: string) => {
    setSelectedItems((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleExecute = async () => {
    if (!bundle) return;
    setIsExecuting(true);
    try {
      const raw = await invoke<ExecutionSummary>("execute_action_bundle", {
        bundleId: bundle.id,
      });
      setExecutionResult(raw);
      await qc.invalidateQueries({ queryKey: ["action-bundles"] });
      await qc.invalidateQueries({ queryKey: ["action-bundle", bundle.id] });
      const { succeeded, failed } = raw;
      if (failed === 0) {
        toast.success(`${succeeded} action${succeeded !== 1 ? "s" : ""} applied successfully`);
      } else {
        toast.error(`${failed} action${failed !== 1 ? "s" : ""} failed`, {
          description: `${succeeded} succeeded`,
        });
      }
    } catch (e) {
      toast.error("Execution failed", { description: String(e) });
    } finally {
      setIsExecuting(false);
    }
  };

  const approvedItems = bundle?.items.filter((i) => i.status === "approved") ?? [];
  const pendingItems = bundle?.items.filter((i) => i.status === "pending") ?? [];
  const totalItems = bundle?.items.length ?? 0;
  const hasExecutable = approvedItems.length > 0 && executionResult === null;

  return (
    <Card className="stack stack-lg" style={{ marginTop: 20 }}>
      {/* Answer */}
      <Card tone="accent" className="stack stack-md">
        <div className="row-sm" style={{ alignItems: "center" }}>
          <I.Brain width={15} height={15} style={{ color: "var(--accent)" }} />
          <span style={{ fontSize: 11.5, fontWeight: 600, color: "var(--accent)", textTransform: "uppercase", letterSpacing: "0.08em", fontFamily: "var(--mono)" }}>
            Copilot
          </span>
          {bundle && <ConfidenceBadge c={bundle.confidence} />}
        </div>
        <p style={{ margin: 0, fontSize: 14, lineHeight: 1.65, color: "var(--ink)" }}>
          {planResult.answer}
        </p>
      </Card>

      {/* Forecast summary */}
      {planResult.forecastSummary && (
        <Card tone="muted" tight>
          {planResult.forecastSummary}
        </Card>
      )}

      {/* Assumptions */}
      {planResult.assumptions.length > 0 && (
        <div className="stack stack-sm">
          <p className="eyebrow">Assumptions</p>
          <ul className="stack stack-xs" style={{ margin: 0, paddingLeft: 20, listStyle: "disc" }}>
            {planResult.assumptions.map((a, i) => (
              <li key={i} className="muted" style={{ fontSize: 12.5 }}>{a}</li>
            ))}
          </ul>
        </div>
      )}

      {/* Follow-up questions */}
      {planResult.followUpQuestions.length > 0 && (
        <div className="stack stack-sm">
          <p className="eyebrow">Clarifying questions</p>
          <div className="row-sm wrap">
            {planResult.followUpQuestions.map((q, i) => (
              <button
                key={i}
                className="chip"
                onClick={() => onFollowUp(q)}
                title="Click to ask this follow-up"
              >
                <I.ArrowRight width={11} height={11} />
                {q}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Action items */}
      {totalItems > 0 && (
        <div className="stack stack-md" role="list" aria-label="Proposed actions">
          <div className="row-md" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <p className="eyebrow" style={{ margin: 0 }}>
              {totalItems} proposed action{totalItems !== 1 ? "s" : ""}
              {pendingItems.length > 0 && (
                <span className="muted" style={{ marginLeft: 8, fontSize: 11 }}>
                  · {pendingItems.length} awaiting review
                </span>
              )}
            </p>
            {pendingItems.length > 0 && (
              <div className="row-xs">
                <Button variant="ghost" size="sm" onClick={() => setSelectedItems(new Set(pendingItems.map((i) => i.id)))}>
                  Select all
                </Button>
                <Button variant="ghost" size="sm" onClick={() => setSelectedItems(new Set())}>
                  Deselect all
                </Button>
              </div>
            )}
          </div>

          {bundle?.items.map((item) => (
            <ActionItemRow
              key={item.id}
              item={item}
              selected={selectedItems.has(item.id)}
              onToggle={toggleItem}
              disabled={isExecuting}
            />
          ))}

          {hasExecutable && (
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <Button
                variant="primary"
                disabled={isExecuting || approvedItems.length === 0}
                loading={isExecuting}
                onClick={() => void handleExecute()}
              >
                {isExecuting ? (
                  <>
                    <span className="spinner" />
                    Executing…
                  </>
                ) : (
                  <>
                    <I.Check width={14} height={14} />
                    Execute {approvedItems.length} approved action{approvedItems.length !== 1 ? "s" : ""}
                  </>
                )}
              </Button>
            </div>
          )}
        </div>
      )}

      {/* Execution results */}
      {executionResult && (
        <div className="stack stack-md" style={{ borderTop: "1px solid var(--hairline)", paddingTop: 16 }}>
          <p className="eyebrow">Execution results</p>
          {executionResult.results.map((r) => (
            <div
              key={r.itemId}
              className="row-sm"
              style={{
                alignItems: "center",
                padding: "8px 12px",
                borderRadius: 6,
                background: r.status === "success" ? "var(--positive-2)" : "var(--negative-2)",
                fontSize: 13,
              }}
            >
              {r.status === "success" ? (
                <I.Check width={13} height={13} style={{ color: "var(--positive)", flexShrink: 0 }} />
              ) : (
                <I.X width={13} height={13} style={{ color: "var(--negative)", flexShrink: 0 }} />
              )}
              <span style={{ color: r.status === "success" ? "var(--positive)" : "var(--negative)" }}>
                {actionKindLabel(r.actionKind)}
              </span>
              {r.summary && (
                <span className="muted" style={{ fontSize: 12 }}>— {r.summary}</span>
              )}
              {r.error && (
                <span style={{ color: "var(--negative)", fontSize: 12 }}>— {r.error}</span>
              )}
            </div>
          ))}
          <div className="muted" style={{ fontSize: 12.5 }}>
            {executionResult.succeeded} succeeded · {executionResult.failed} failed
          </div>
        </div>
      )}

      {/* Execution log from DB */}
      {execLog && execLog.length > 0 && !executionResult && (
        <div className="stack stack-sm" style={{ borderTop: "1px solid var(--hairline)", paddingTop: 14 }}>
          <p className="eyebrow">Execution history</p>
          {execLog.slice(0, 5).map((entry) => (
            <div key={entry.id} className="row-sm" style={{ alignItems: "center", padding: "6px 0", borderBottom: "1px solid var(--hairline)", fontSize: 12.5 }}>
              <span style={{ color: entry.status === "success" ? "var(--positive)" : "var(--negative)" }}>
                {entry.status === "success" ? "✓" : "✗"}
              </span>
              <span>{actionKindLabel(entry.actionKind)}</span>
              <span className="num muted" style={{ marginLeft: "auto", fontSize: 11.5 }}>
                {new Date(entry.executedAt).toLocaleTimeString()}
              </span>
            </div>
          ))}
        </div>
      )}
    </Card>
  );
}

// ── Past bundles section ────────────────────────────────────────────────────

function PastBundlesSection() {
  const { data: bundles, isLoading } = useActionBundles(null, 10);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const { data: expandedBundle } = useActionBundle(expandedId);

  if (isLoading) return null;
  if (!bundles || bundles.length === 0) return null;

  return (
    <section className="stack stack-md" style={{ marginTop: 36 }}>
      <p className="eyebrow">
        <I.Flow width={13} height={13} />
        Recent plans &amp; bundles
      </p>
      <Card flush>
        {bundles.map((bundle, idx) => (
          <div key={bundle.id}>
            <Button
              variant="ghost"
              className="row-md"
              style={{
                width: "100%",
                justifyContent: "space-between",
                padding: "12px 18px",
                background: expandedId === bundle.id ? "var(--surface-2)" : "transparent",
                textAlign: "left",
                gap: 12,
                borderRadius: 0,
              }}
              onClick={() => setExpandedId(expandedId === bundle.id ? null : bundle.id)}
            >
              <span className="row-sm" style={{ minWidth: 0 }}>
                <I.Brain width={13} height={13} style={{ color: "var(--ink-faint)", flexShrink: 0 }} />
                <span style={{ fontSize: 13.5, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {bundle.title}
                </span>
              </span>
              <span className="row-sm" style={{ flexShrink: 0 }}>
                <Badge tone={bundle.status === "executed" ? "positive" : bundle.status === "pending" ? "warning" : "default"}>
                  {bundle.status}
                </Badge>
                <span className="num muted" style={{ fontSize: 11.5 }}>
                  {new Date(bundle.createdAt).toLocaleDateString()}
                </span>
                {expandedId === bundle.id ? <I.Up width={12} height={12} /> : <I.Down width={12} height={12} />}
              </span>
            </Button>

            {expandedId === bundle.id && expandedBundle && (
              <div className="stack stack-sm" style={{ padding: "0 18px 16px", borderTop: "1px solid var(--hairline)" }}>
                <p className="muted" style={{ margin: "12px 0 10px", fontSize: 13 }}>
                  {expandedBundle.summary}
                </p>
                {expandedBundle.items.map((item) => (
                  <div key={item.id} className="row-sm" style={{ alignItems: "center", padding: "6px 0", borderBottom: "1px solid var(--hairline)", fontSize: 12.5 }}>
                    <span className="muted" aria-hidden="true">
                      {actionKindIcon(item.actionKind)}
                    </span>
                    <span>{actionKindLabel(item.actionKind)}</span>
                    <ItemStatusBadge status={item.status} />
                    <span className="muted" style={{ marginLeft: "auto", fontSize: 11.5 }}>
                      {item.rationale.slice(0, 60)}{item.rationale.length > 60 ? "…" : ""}
                    </span>
                  </div>
                ))}
              </div>
            )}

            {idx < bundles.length - 1 && (
              <hr className="divider" />
            )}
          </div>
        ))}
      </Card>
    </section>
  );
}

// ── Main screen ─────────────────────────────────────────────────────────────

export default function Copilot() {
  const [question, setQuestion] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [planResult, setPlanResult] = useState<CopilotPlanResult | null>(null);
  const [activeBundleId, setActiveBundleId] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { data: activeBundle } = useActionBundle(activeBundleId);
  const qc = useQueryClient();

  // Auto-resize textarea
  const handleInput = () => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 200) + "px";
  };

  // Pick up pre-filled prompt from CopilotNudge navigation
  useEffect(() => {
    const prefill = sessionStorage.getItem("copilot.prefill");
    if (prefill) {
      sessionStorage.removeItem("copilot.prefill");
      setQuestion(prefill);
      setTimeout(() => void handleAsk(prefill), 100);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleAsk = async (q: string = question) => {
    const trimmed = q.trim();
    if (!trimmed) return;

    setIsLoading(true);
    setPlanResult(null);
    setActiveBundleId(null);
    setQuestion("");

    try {
      const raw = await invoke<CopilotPlanResult>("start_copilot_plan", {
        sessionId: null,
        question: trimmed,
      });
      setPlanResult(raw);
      setActiveBundleId(raw.bundleId);
      await qc.invalidateQueries({ queryKey: ["action-bundles"] });
    } catch (e) {
      const err = e as { message?: string };
      const msg = err?.message ?? String(e);
      if (msg.includes("no_provider") || msg.includes("Configure an AI provider")) {
        toast.error("AI provider not configured", {
          description: "Go to Settings → Agent to set up your AI provider.",
          action: { label: "Settings", onClick: () => { window.location.hash = "/settings"; } },
        });
      } else {
        toast.error("Copilot request failed", { description: msg });
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      void handleAsk();
    }
  };

  return (
    <div className="screen">
      {/* Header */}
      <header className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <I.Brain width={12} height={12} />
            Your personal financial analyst
          </div>
          <h1>Copilot</h1>
        </div>
        <div className="row-sm">
          <span
            className="chip"
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
              padding: "5px 12px",
            }}
          >
            <span className="dot" aria-hidden="true" />
            AI-assisted · local provider
          </span>
        </div>
      </header>

      {/* Ask bar */}
      <Card
        style={{
          padding: 0,
          overflow: "hidden",
          borderColor: isLoading ? "var(--accent-3)" : undefined,
        }}
      >
        <textarea
          ref={textareaRef}
          value={question}
          onChange={(e) => { setQuestion(e.target.value); handleInput(); }}
          onKeyDown={handleKeyDown}
          placeholder={'Ask your financial analyst anything\u2026 e.g., "How can I reach my goals faster?"'}
          disabled={isLoading}
          rows={2}
          style={{
            width: "100%",
            resize: "none",
            background: "transparent",
            border: "none",
            outline: "none",
            padding: "18px 20px 12px",
            fontSize: 14.5,
            color: "var(--ink)",
            fontFamily: "var(--sans)",
            lineHeight: 1.55,
            boxSizing: "border-box",
          }}
        />
        <div
          className="row-md"
          style={{
            justifyContent: "space-between",
            padding: "8px 12px 12px",
            gap: 8,
          }}
        >
          {/* Suggested prompts */}
          <div className="row-sm wrap" style={{ flex: 1, minWidth: 0 }}>
            {SUGGESTED_PROMPTS.slice(0, 4).map((p) => (
              <button
                key={p}
                className="chip"
                style={{ cursor: "pointer", fontSize: 11.5 }}
                disabled={isLoading}
                onClick={() => {
                  setQuestion(p);
                  textareaRef.current?.focus();
                }}
              >
                {p}
              </button>
            ))}
          </div>

          {/* Submit */}
          <Button
            variant="primary"
            disabled={isLoading || !question.trim()}
            loading={isLoading}
            onClick={() => void handleAsk()}
            title="Ask Copilot (⌘↵)"
            style={{ flexShrink: 0 }}
          >
            {isLoading ? (
              <>
                <span className="spinner" />
                Thinking…
              </>
            ) : (
              <>
                <I.Send width={14} height={14} />
                Ask Copilot
                <span className="kbd">⌘↵</span>
              </>
            )}
          </Button>
        </div>
      </Card>

      {/* More prompts row */}
      <div className="row-sm wrap" style={{ marginTop: 10 }}>
        {SUGGESTED_PROMPTS.slice(4).map((p) => (
          <button
            key={p}
            className="chip"
            style={{ cursor: "pointer", fontSize: 11.5 }}
            disabled={isLoading}
            onClick={() => void handleAsk(p)}
          >
            <I.ArrowRight width={11} height={11} />
            {p}
          </button>
        ))}
      </div>

      {/* Loading state */}
      {isLoading && (
        <Card className="row-md" style={{ marginTop: 20, padding: "24px 28px" }}>
          <I.Brain width={16} height={16} style={{ color: "var(--accent)" }} />
          <div className="stack stack-xs">
            <div style={{ fontSize: 14, fontWeight: 500 }}>
              Analyzing your finances…
            </div>
            <div className="muted" style={{ fontSize: 12.5 }}>
              Building context · calling AI analyst · preparing recommendations
            </div>
          </div>
        </Card>
      )}

      {/* Plan result */}
      {planResult && !isLoading && (
        <PlanCard
          planResult={planResult}
          bundle={activeBundle}
          onFollowUp={(q) => {
            setQuestion(q);
            textareaRef.current?.focus();
          }}
        />
      )}

      {/* Past bundles */}
      <PastBundlesSection />

      {/* Empty state */}
      {!isLoading && !planResult && (
        <EmptyState
          icon={<I.Brain style={{ color: "var(--ink-faint)", width: 48, height: 48 }} />}
          title="Your personal financial analyst"
          description="Ask anything about your finances. The Copilot analyzes your spending, budget, goals, and recurring bills to give you personalized recommendations and action plans."
          actions={
            <div className="row-sm wrap" style={{ justifyContent: "center" }}>
              {["Plan", "Save", "Forecast", "Reduce", "Clean up", "Explain"].map((label) => (
                <Badge key={label} tone="accent">{label}</Badge>
              ))}
            </div>
          }
        />
      )}
    </div>
  );
}
