import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import * as I from "./Icons";
import { useAskAgent } from "../api/hooks/agent";
import type { AgentAnswer } from "../api/client";

// ── Types ─────────────────────────────────────────────────────────────────

type PaletteMode = "list" | "answer";

interface CmdItem {
  kind: "nav" | "act" | "ask";
  label: string;
  path?: string;
  hint?: string;
  Icon?: React.FC<React.SVGProps<SVGSVGElement>>;
  query?: string;  // only for kind === "ask"
}

// ── Static lists ──────────────────────────────────────────────────────────

const NAV_ITEMS: CmdItem[] = [
  { kind: "nav", label: "Go to Today",        path: "/",             Icon: I.Today },
  { kind: "nav", label: "Go to Copilot",      path: "/copilot",      Icon: I.Brain },
  { kind: "nav", label: "Go to Insights",     path: "/insights",     Icon: I.Sparkle },
  { kind: "nav", label: "Go to Accounts",     path: "/accounts",     Icon: I.Wallet },
  { kind: "nav", label: "Go to Transactions", path: "/transactions", Icon: I.Flow },
  { kind: "nav", label: "Go to Budget",       path: "/budget",       Icon: I.Lego },
  { kind: "nav", label: "Go to Categories",   path: "/categories",   Icon: I.Grid },
  { kind: "nav", label: "Go to Recurring",    path: "/recurring",    Icon: I.Repeat },
  { kind: "nav", label: "Go to Goals",        path: "/goals",        Icon: I.Goal },
  { kind: "nav", label: "Go to Scenarios",    path: "/scenarios",    Icon: I.ArrowRight },
  { kind: "nav", label: "Go to Reports",      path: "/reports",      Icon: I.Spark },
  { kind: "nav", label: "Go to Rules",        path: "/rules",        Icon: I.Bolt },
  { kind: "nav", label: "Go to Settings",     path: "/settings",     Icon: I.Gear },
];

const ACT_ITEMS: CmdItem[] = [
  { kind: "act", label: "Add a transaction…",        Icon: I.Plus,     hint: "manual" },
  { kind: "act", label: "Toggle privacy mode",        Icon: I.EyeOff,   hint: "⌘." },
  { kind: "act", label: "Run a what-if scenario",     Icon: I.Bolt,     path: "/scenarios" },
  { kind: "nav", label: "Plan next month with Copilot", path: "/copilot", Icon: I.Brain },
];

// ── Main component ────────────────────────────────────────────────────────

interface Props { open: boolean; onClose: () => void; }

export function CommandPalette({ open, onClose }: Props) {
  const navigate = useNavigate();
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [mode, setMode] = useState<PaletteMode>("list");
  const [answer, setAnswer] = useState<AgentAnswer | null>(null);
  const [activeQuery, setActiveQuery] = useState<string>("");
  const inputRef = useRef<HTMLInputElement>(null);
  const askAgent = useAskAgent();

  // Reset on open/close
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

  // Build item list: ask item (when query non-empty) + nav + act
  const trimmed = q.trim();
  const askItem: CmdItem | null = trimmed
    ? { kind: "ask", label: `Ask: ${trimmed}`, Icon: I.Sparkle, query: trimmed }
    : null;

  const filtered: CmdItem[] = [];
  if (askItem && !trimmed) {
    // no ask item if empty
  } else if (trimmed) {
    if (askItem) filtered.push(askItem);
    const s = trimmed.toLowerCase();
    filtered.push(...[...NAV_ITEMS, ...ACT_ITEMS].filter((x) => x.label.toLowerCase().includes(s)));
  } else {
    filtered.push(...NAV_ITEMS, ...ACT_ITEMS);
  }

  useEffect(() => { setSel(0); }, [q]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (mode === "answer") { setMode("list"); return; }
        onClose();
        return;
      }
      if (mode === "answer") return;
      if (e.key === "ArrowDown") { e.preventDefault(); setSel((s) => Math.min(filtered.length - 1, s + 1)); }
      if (e.key === "ArrowUp")   { e.preventDefault(); setSel((s) => Math.max(0, s - 1)); }
      if (e.key === "Enter") {
        e.preventDefault();
        const item = filtered[sel];
        if (item) handleItem(item);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, filtered, sel, mode]);

  const handleItem = (item: CmdItem) => {
    if (item.kind === "ask" && item.query) {
      setActiveQuery(item.query);
      setMode("answer");
      setAnswer(null);
      askAgent.mutate({ question: item.query }, {
        onSuccess: (data) => setAnswer(data),
        onError: (err) => {
          const isNoProvider = err.message.includes("no_provider");
          setAnswer({
            prose: isNoProvider
              ? "No AI provider configured. Set one up in Settings → Agent to use this feature."
              : `Something went wrong: ${err.message}`,
            reasoning: "",
            trace: [],
            changes: [],
            actionLabel: isNoProvider ? "Open Settings →" : null,
            actionPath: isNoProvider ? "/settings" : null,
          });
        },
      });
      return;
    }
    if (item.path) { navigate(item.path); onClose(); }
    else { onClose(); }
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
        <div key={myIdx} className={`cmdk-item${isSel ? " sel" : ""}`}
          onMouseEnter={() => setSel(myIdx)}
          onClick={() => handleItem(item)}>
          {Icon ? <Icon className="ico" /> : <span className="ico" />}
          <span>{item.label}</span>
          {item.hint && <span className="hint">{item.hint}</span>}
        </div>
      );
    });

  return (
    <div className="cmdk-mask" onClick={onClose} role="dialog" aria-modal="true" aria-label="Command palette">
      <div
        className="cmdk"
        style={mode === "answer" ? { maxWidth: "min(760px, 94vw)" } : undefined}
        onClick={(e) => e.stopPropagation()}
      >
        {mode === "answer" ? (
          // ── Answer mode ──────────────────────────────────────────────────
          <>
            <div className="cmdk-input" style={{ borderBottom: "1px solid var(--hairline)" }}>
              <I.Sparkle style={{ color: "var(--accent)", width: 16, height: 16, flexShrink: 0 }} />
              <span style={{ flex: 1, fontSize: 14, color: "var(--ink-mute)" }}>{activeQuery}</span>
              <button className="btn sm ghost" onClick={() => setMode("list")} style={{ fontSize: 12 }}>
                ← Back
              </button>
            </div>
            <div style={{ padding: "20px 24px" }}>
              {askAgent.isPending && !answer ? (
                <div style={{ display: "flex", alignItems: "center", gap: 10, color: "var(--ink-mute)", fontSize: 14 }}>
                  <span style={{ display: "inline-block", width: 14, height: 14, border: "2px solid var(--accent)",
                    borderTopColor: "transparent", borderRadius: "50%", animation: "spin 0.7s linear infinite" }} />
                  Thinking…
                </div>
              ) : answer ? (
                <>
                  <p style={{ marginBottom: 16, fontSize: 14, lineHeight: 1.6 }}>{answer.prose}</p>
                  {answer.actionLabel && answer.actionPath && (
                    <div style={{ marginTop: 16 }}>
                      <button className="btn primary sm" onClick={() => { navigate(answer.actionPath!); onClose(); }}>
                        {answer.actionLabel}
                      </button>
                    </div>
                  )}
                </>
              ) : null}
            </div>
          </>
        ) : (
          // ── List mode ────────────────────────────────────────────────────
          <>
            <div className="cmdk-input">
              <I.Search style={{ color: "var(--ink-mute)", width: 16, height: 16, flexShrink: 0 }} />
              <input
                ref={inputRef}
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder="Search, jump to page, or ask the agent…"
                aria-label="Command palette input"
              />
              <span className="kbd" style={{ fontFamily: "var(--mono)" }}>esc</span>
            </div>
            <div className="cmdk-list" role="listbox">
              {filtered.length === 0 && !trimmed && (
                <div className="cmdk-item" style={{ color: "var(--ink-mute)" }}>
                  Start typing to search or ask the agent a question
                </div>
              )}
              {/* Ask the agent section — only shown when query is non-empty */}
              {trimmed && askItem && (
                <>
                  <div className="cmdk-section">Ask the agent</div>
                  {(() => { const myIdx = idx++; const isSel = myIdx === sel; return (
                    <div key="ask" className={`cmdk-item${isSel ? " sel" : ""}`}
                      onMouseEnter={() => setSel(myIdx)}
                      onClick={() => handleItem(askItem)}>
                      <I.Sparkle className="ico" style={{ color: "var(--accent)" }} />
                      <span>{askItem.label}</span>
                    </div>
                  ); })()}
                </>
              )}
              {/* Filtered nav/act when searching */}
              {trimmed && matchedF.filter((x) => x.kind !== "ask").length > 0 && (
                <>
                  <div className="cmdk-section">Jump to / Actions</div>
                  {renderItems(matchedF.filter((x) => x.kind !== "ask"))}
                </>
              )}
              {/* Default sections when not searching */}
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
              <span><span className="k">↑↓</span> navigate</span>
              <span><span className="k">↵</span> select</span>
              <span><span className="k">esc</span> close</span>
            </div>
          </>
        )}
      </div>
    </div>
  );
}


