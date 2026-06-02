import { useEffect, useRef, useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import * as I from "./Icons";

interface CmdItem {
  kind: "nav" | "act";
  label: string;
  path?: string;
  hint?: string;
  Icon?: React.FC<React.SVGProps<SVGSVGElement>>;
}

const NAV_ITEMS: CmdItem[] = [
  { kind: "nav", label: "Go to Today",        path: "/",             Icon: I.Today },
  { kind: "nav", label: "Go to Accounts",     path: "/accounts",     Icon: I.Wallet },
  { kind: "nav", label: "Go to Transactions", path: "/transactions", Icon: I.Flow },
  { kind: "nav", label: "Go to Budget",       path: "/budget",       Icon: I.Lego },
  { kind: "nav", label: "Go to Categories",   path: "/categories",   Icon: I.Grid },
  { kind: "nav", label: "Go to Recurring",    path: "/recurring",    Icon: I.Repeat },
  { kind: "nav", label: "Go to Goals",        path: "/goals",        Icon: I.Goal },
  { kind: "nav", label: "Go to Reports",      path: "/reports",      Icon: I.Spark },
  { kind: "nav", label: "Go to Rules",        path: "/rules",        Icon: I.Bolt },
  { kind: "nav", label: "Go to Settings",     path: "/settings",     Icon: I.Gear },
];

const ACT_ITEMS: CmdItem[] = [
  { kind: "act", label: "Add a transaction…",  Icon: I.Plus,   hint: "manual" },
  { kind: "act", label: "Toggle privacy mode", Icon: I.EyeOff, hint: "⌘." },
];

interface Props {
  open: boolean;
  onClose: () => void;
}

export function CommandPalette({ open, onClose }: Props) {
  const navigate = useNavigate();
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus input when opened; reset state
  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 30);
      setQ("");
      setSel(0);
    }
  }, [open]);

  const all = useMemo(() => [...NAV_ITEMS, ...ACT_ITEMS], []);

  const filtered = useMemo(() => {
    if (!q.trim()) return all;
    const s = q.toLowerCase();
    return all.filter((x) => x.label.toLowerCase().includes(s));
  }, [q, all]);

  useEffect(() => {
    setSel(0);
  }, [q]);

  // Keyboard navigation
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { onClose(); return; }
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
  }, [open, filtered, sel]);

  const handleItem = (item: CmdItem) => {
    if (item.path) {
      navigate(item.path);
      onClose();
    } else {
      onClose();
    }
  };

  if (!open) return null;

  const navsF = filtered.filter((x) => x.kind === "nav");
  const actsF = filtered.filter((x) => x.kind === "act");

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
          onMouseEnter={() => setSel(myIdx)}
          onClick={() => handleItem(item)}
        >
          {Icon ? <Icon className="ico" /> : <span className="ico" />}
          <span>{item.label}</span>
          {item.hint && <span className="hint">{item.hint}</span>}
        </div>
      );
    });

  return (
    <div className="cmdk-mask" onClick={onClose} role="dialog" aria-modal="true" aria-label="Command palette">
      <div className="cmdk" onClick={(e) => e.stopPropagation()}>
        {/* Input */}
        <div className="cmdk-input">
          <I.Search style={{ color: "var(--ink-mute)", width: 16, height: 16, flexShrink: 0 }} />
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search, jump to page, or type a command…"
            aria-label="Command palette input"
          />
          <span className="kbd" style={{ fontFamily: "var(--mono)" }}>esc</span>
        </div>

        {/* Results */}
        <div className="cmdk-list" role="listbox">
          {filtered.length === 0 && (
            <div className="cmdk-item" style={{ color: "var(--ink-mute)" }}>
              No results for "{q}"
            </div>
          )}
          {navsF.length > 0 && (
            <>
              <div className="cmdk-section">Jump to</div>
              {renderItems(navsF)}
            </>
          )}
          {actsF.length > 0 && (
            <>
              <div className="cmdk-section">Actions</div>
              {renderItems(actsF)}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="cmdk-foot">
          <span><span className="k">↑↓</span> navigate</span>
          <span><span className="k">↵</span> select</span>
          <span><span className="k">esc</span> close</span>
        </div>
      </div>
    </div>
  );
}
