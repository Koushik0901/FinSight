import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import * as I from "./Icons";
import { useAskAgent } from "../api/hooks/agent";
import type { AgentAnswer } from "../api/client";
import { userErrorMessage } from "../utils/runtime";
import { AgentResponseRenderer } from "./AgentResponseRenderer";

// ── Types ─────────────────────────────────────────────────────────────────

type PaletteMode = "list" | "answer";

interface CmdItem {
  kind: "nav" | "act" | "ask";
  label: string;
  path?: string;
  hint?: string;
  Icon?: React.FC<React.SVGProps<SVGSVGElement>>;
  query?: string;
}

// ── Static lists ──────────────────────────────────────────────────────────

const NAV_ITEMS: CmdItem[] = [
  { kind: "nav", label: "Go to Today", path: "/", Icon: I.Today },
  { kind: "nav", label: "Go to Copilot", path: "/copilot", Icon: I.Brain },
  { kind: "nav", label: "Go to Insights", path: "/insights", Icon: I.Sparkle },
  { kind: "nav", label: "Go to Accounts", path: "/accounts", Icon: I.Wallet },
  { kind: "nav", label: "Go to Budget", path: "/budget", Icon: I.Lego },
  { kind: "nav", label: "Go to Categories", path: "/categories", Icon: I.Grid },
  { kind: "nav", label: "Go to Recurring", path: "/recurring", Icon: I.Repeat },
  { kind: "nav", label: "Go to Goals", path: "/goals", Icon: I.Goal },
  { kind: "nav", label: "Go to Scenarios", path: "/scenarios", Icon: I.ArrowRight },
  { kind: "nav", label: "Go to Reports", path: "/reports", Icon: I.Spark },
  { kind: "nav", label: "Go to Rules", path: "/rules", Icon: I.Bolt },
  { kind: "nav", label: "Go to Settings", path: "/settings", Icon: I.Gear },
];

const ACT_ITEMS: CmdItem[] = [
  { kind: "act", label: "Add a transaction…", Icon: I.Plus, hint: "manual" },
  { kind: "act", label: "Toggle privacy mode", Icon: I.EyeOff, hint: "⌘." },
  { kind: "act", label: "Run a what-if scenario", Icon: I.Bolt, path: "/scenarios" },
  { kind: "nav", label: "Plan next month with Copilot", path: "/copilot", Icon: I.Brain },
];

// ── Main component ────────────────────────────────────────────────────────

interface Props {
  open: boolean;
  onClose: () => void;
}

export function CommandPalette({ open, onClose }: Props) {
  const navigate = useNavigate();
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [mode, setMode] = useState<PaletteMode>("list");
  const [answer, setAnswer] = useState<AgentAnswer | null>(null);
  const [activeQuery, setActiveQuery] = useState<string>("");
  const inputRef = useRef<HTMLInputElement>(null);
  const askAgent = useAskAgent();

  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 30);
      setQ("");
      setSel(0);
      setMode("list");
      setAnswer(null);
      setActiveQuery("");
    }
  }, [open]);

  const trimmed = q.trim();
  const askItem: CmdItem | null = trimmed
    ? { kind: "ask", label: `Ask: ${trimmed}`, Icon: I.Sparkle, query: trimmed }
    : null;

  const filtered: CmdItem[] = [];
  if (trimmed) {
    const s = trimmed.toLowerCase();
    const matches = [...NAV_ITEMS, ...ACT_ITEMS].filter((x) => x.label.toLowerCase().includes(s));
    filtered.push(...matches);
    if (askItem) filtered.push(askItem);
  } else {
    filtered.push(...NAV_ITEMS, ...ACT_ITEMS);
  }

  useEffect(() => {
    setSel(0);
  }, [q]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (mode === "answer") {
          setMode("list");
          return;
        }
        onClose();
        return;
      }
      if (mode === "answer") return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSel((s) => Math.min(filtered.length - 1, s + 1));
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSel((s) => Math.max(0, s - 1));
      }
      if (e.key === "Enter") {
        e.preventDefault();
        const item = filtered[sel];
        if (item) handleItem(item);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, filtered, sel, mode, onClose]);

  const handleItem = (item: CmdItem) => {
    if (item.kind === "ask" && item.query) {
      setActiveQuery(item.query);
      setMode("answer");
      setAnswer(null);
      askAgent.mutate(
        { question: item.query },
        {
          onSuccess: (data) => setAnswer(data),
          onError: (err) => {
            const isNoProvider = err.message.includes("no_provider");
            setAnswer({
              prose: isNoProvider
                ? "No AI provider configured. Set one up in Settings → Agent to use this feature."
                : userErrorMessage(err, "Copilot could not answer right now. Try again from the desktop app."),
              reasoning: "",
              plan: [],
              trace: [],
              changes: [],
              actionLabel: isNoProvider ? "Open Settings →" : null,
              actionPath: isNoProvider ? "/settings" : null,
              bundleId: null,
              assumptions: [],
              dataSources: [],
              missingData: [],
              alternatives: [],
              followUpQuestions: [],
              responseBlocks: [],
            });
          },
        }
      );
      return;
    }
    if (item.kind === "act" && item.label === "Toggle privacy mode") {
      onClose();
      return;
    }
    if (item.path) {
      navigate(item.path);
      onClose();
    } else {
      onClose();
    }
  };

  if (!open) return null;

  const navF = trimmed ? [] : filtered.filter((x) => x.kind === "nav");
  const actF = trimmed ? [] : filtered.filter((x) => x.kind === "act");
  const matchedF = trimmed ? filtered : [];

  let idx = 0;
  const renderItems = (items: CmdItem[]) =>
    items.map((item) => {
      const myIdx = idx++;
      const isSel = myIdx === sel;
      const Icon = item.Icon;
      return (
        <div
          key={myIdx}
          className={`cmdk-item${isSel ? " sel" : ""}`}
          role="option"
          aria-selected={isSel}
          onMouseEnter={() => setSel(myIdx)}
          onClick={() => handleItem(item)}
        >
          {Icon ? <Icon className="ico" aria-hidden="true" /> : <span className="ico" />}
          <span>{item.label}</span>
          {item.hint && <span className="hint">{item.hint}</span>}
        </div>
      );
    });

  return (
    <div className="cmdk-mask" onClick={onClose} role="dialog" aria-modal="true" aria-label="Command palette">
      <div className={`cmdk${mode === "answer" ? " answer" : ""}`} onClick={(e) => e.stopPropagation()}>
        {mode === "answer" ? (
          <>
            <div className="cmdk-input cmdk-answer-header">
              <I.Sparkle className="ico accent" aria-hidden="true" />
              <span className="cmdk-answer-query">{activeQuery}</span>
              <button type="button" className="btn sm ghost" onClick={() => setMode("list")}>
                ← Back
              </button>
            </div>
            <div className="cmdk-answer-body">
              {askAgent.isPending && !answer ? (
                <div className="cmdk-thinking">
                  <span className="spinner" aria-hidden="true" />
                  <span>Thinking…</span>
                </div>
              ) : answer ? (
                <>
                  <AgentResponseRenderer answer={answer} compact />
                  {answer.actionLabel && answer.actionPath && (
                    <div className="cmdk-answer-action">
                      <button
                        type="button"
                        className="btn primary sm"
                        onClick={() => {
                          navigate(answer.actionPath!);
                          onClose();
                        }}
                      >
                        {answer.actionLabel}
                      </button>
                    </div>
                  )}
                </>
              ) : null}
            </div>
          </>
        ) : (
          <>
            <div className="cmdk-input">
              <I.Search className="ico" aria-hidden="true" />
              <input
                ref={inputRef}
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder="Search, jump to page, or ask the agent…"
                aria-label="Command palette input"
              />
              <span className="kbd">esc</span>
            </div>
            <div className="cmdk-list" role="listbox">
              {filtered.length === 0 && !trimmed && (
                <div className="cmdk-item muted">Start typing to search or ask the agent a question</div>
              )}
              {trimmed && matchedF.filter((x) => x.kind !== "ask").length > 0 && (
                <>
                  <div className="cmdk-section">Jump to / Actions</div>
                  {renderItems(matchedF.filter((x) => x.kind !== "ask"))}
                </>
              )}
              {trimmed && matchedF.filter((x) => x.kind === "ask").length > 0 && (
                <>
                  <div className="cmdk-section">Ask the agent</div>
                  {renderItems(matchedF.filter((x) => x.kind === "ask"))}
                </>
              )}
              {!trimmed && navF.length > 0 && (
                <>
                  <div className="cmdk-section">Jump to</div>
                  {renderItems(navF)}
                </>
              )}
              {!trimmed && actF.length > 0 && (
                <>
                  <div className="cmdk-section">Actions</div>
                  {renderItems(actF)}
                </>
              )}
            </div>
            <div className="cmdk-foot">
              <span>
                <span className="k">↑↓</span> navigate
              </span>
              <span>
                <span className="k">↵</span> select
              </span>
              <span>
                <span className="k">esc</span> close
              </span>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
