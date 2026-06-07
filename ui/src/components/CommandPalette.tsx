import { useEffect, useRef, useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import * as I from "./Icons";
import { commands, type MonthTotals } from "../api/client";
import { useNetWorth } from "../api/hooks/networth";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import { money } from "../utils/format";

// ── Types ─────────────────────────────────────────────────────────────────

interface BigNumberData { value: string; label: string; }
interface CompareBarsData { thisMonth: number; lastMonth: number; }
interface ProgressData { label: string; pct: number; }

interface CannedQuestion {
  label: string;
  prose: string;
  kind: "bigNumber" | "compareBars" | "progress";
  vizData: BigNumberData | CompareBarsData | ProgressData;
  actionLabel?: string;
  actionPath?: string;
}

type PaletteMode = "list" | "answer";

interface CmdItem {
  kind: "nav" | "act" | "ask";
  label: string;
  path?: string;
  hint?: string;
  Icon?: React.FC<React.SVGProps<SVGSVGElement>>;
  question?: CannedQuestion;  // only for kind === "ask"
}

// ── Static lists ──────────────────────────────────────────────────────────

const NAV_ITEMS: CmdItem[] = [
  { kind: "nav", label: "Go to Today",        path: "/",             Icon: I.Today },
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
];

// ── AskViz component ──────────────────────────────────────────────────────

function AskViz({ question }: { question: CannedQuestion }) {
  if (question.kind === "bigNumber") {
    const d = question.vizData as BigNumberData;
    return (
      <div style={{ textAlign: "center", padding: "24px 0" }}>
        <div className="figure" style={{ fontSize: 52, lineHeight: 1, color: "var(--accent)", marginBottom: 8 }}>
          {d.value}
        </div>
        <div className="muted" style={{ fontSize: 14 }}>{d.label}</div>
      </div>
    );
  }
  if (question.kind === "compareBars") {
    const d = question.vizData as CompareBarsData;
    const max = Math.max(d.thisMonth, d.lastMonth, 1);
    return (
      <div style={{ padding: "16px 0", display: "flex", flexDirection: "column", gap: 10 }}>
        {[
          { label: "This month", value: d.thisMonth },
          { label: "Last month", value: d.lastMonth },
        ].map(({ label, value }) => (
          <div key={label}>
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12.5,
              color: "var(--ink-mute)", marginBottom: 4 }}>
              <span>{label}</span>
              <span className="num">{money(value)}</span>
            </div>
            <div style={{ height: 8, background: "var(--surface-2)", borderRadius: 999 }}>
              <div style={{ width: `${(value / max) * 100}%`, height: "100%",
                background: "var(--accent)", borderRadius: 999 }} />
            </div>
          </div>
        ))}
      </div>
    );
  }
  if (question.kind === "progress") {
    const d = question.vizData as ProgressData;
    const over = d.pct > 100;
    return (
      <div style={{ padding: "16px 0" }}>
        <div style={{ display: "flex", justifyContent: "space-between", fontSize: 13,
          marginBottom: 6 }}>
          <span>{d.label}</span>
          <span className="num" style={{ color: over ? "var(--negative)" : undefined }}>
            {Math.round(d.pct)}%
          </span>
        </div>
        <div style={{ height: 10, background: "var(--surface-2)", borderRadius: 999 }}>
          <div style={{ width: `${Math.min(100, d.pct)}%`, height: "100%",
            background: over ? "var(--negative)" : "var(--accent)", borderRadius: 999 }} />
        </div>
      </div>
    );
  }
  return null;
}

// ── Main component ────────────────────────────────────────────────────────

interface Props { open: boolean; onClose: () => void; }

export function CommandPalette({ open, onClose }: Props) {
  const navigate = useNavigate();
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [mode, setMode] = useState<PaletteMode>("list");
  const [activeQ, setActiveQ] = useState<CannedQuestion | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Data for canned questions — fetched when palette opens
  const { data: totals } = useQuery<MonthTotals>({
    queryKey: ["month-totals"],
    queryFn: async () => {
      const r = await commands.getMonthTotals();
      if (r.status === "error") throw new Error(r.error.message);
      return r.data;
    },
    enabled: open,
    staleTime: 60_000,
  });
  const { data: cats = [] } = useCategoriesWithSpending();
  const netWorth = useNetWorth();

  // Reset on open/close
  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 30);
      setQ("");
      setSel(0);
      setMode("list");
      setActiveQ(null);
    }
  }, [open]);

  // Compute canned questions from real data
  const questions = useMemo<CannedQuestion[]>(() => {
    if (!totals || cats.length === 0) return [];
    const topCat = [...cats].sort((a, b) => b.thisMonthCents - a.thisMonthCents)[0];
    const lastMonthTotal = cats.reduce((s, c) => s + c.lastMonthCents, 0);
    const overBudget = cats.filter((c) => c.budgetCents > 0)
      .sort((a, b) => (b.thisMonthCents / b.budgetCents) - (a.thisMonthCents / a.budgetCents))[0];
    const dayOfMonth = new Date().getDate();
    const avgDailyBurn = totals.expenseCents / dayOfMonth;
    const runwayDays = avgDailyBurn > 0 ? Math.max(0, Math.round(netWorth / avgDailyBurn)) : null;

    return [
      {
        label: "What's my top spending category this month?",
        prose: topCat
          ? `Your biggest expense category is ${topCat.label} at ${money(topCat.thisMonthCents)}.`
          : "No spending data yet.",
        kind: "bigNumber",
        vizData: { value: money(topCat?.thisMonthCents ?? 0), label: topCat?.label ?? "—" },
        actionLabel: "Open Categories →",
        actionPath: "/categories",
      },
      {
        label: "How does my spending compare to last month?",
        prose: `This month: ${money(totals.expenseCents)}. Last month: ${money(lastMonthTotal)}.`,
        kind: "compareBars",
        vizData: { thisMonth: totals.expenseCents, lastMonth: lastMonthTotal },
        actionLabel: "Open Reports →",
        actionPath: "/reports",
      },
      {
        label: "What's my current savings rate?",
        prose: `You're keeping ${totals.savingsRatePct}% of your income this month.`,
        kind: "bigNumber",
        vizData: { value: `${totals.savingsRatePct}%`, label: "of income kept" },
        actionLabel: "Open Today →",
        actionPath: "/",
      },
      {
        label: "Which category am I closest to maxing out?",
        prose: overBudget
          ? `${overBudget.label} is at ${Math.round((overBudget.thisMonthCents / overBudget.budgetCents) * 100)}% of budget.`
          : "No budgets set yet.",
        kind: "progress",
        vizData: overBudget
          ? { label: overBudget.label, pct: Math.min(120, (overBudget.thisMonthCents / overBudget.budgetCents) * 100) }
          : { label: "—", pct: 0 },
        actionLabel: "Open Budget →",
        actionPath: "/budget",
      },
      {
        label: "What's my financial runway?",
        prose: runwayDays !== null
          ? `At your current burn rate, you have ${runwayDays} days of runway.`
          : "Not enough spending data to estimate runway.",
        kind: "bigNumber",
        vizData: { value: runwayDays !== null ? runwayDays.toLocaleString() : "—", label: "days runway" },
        actionLabel: "Open Accounts →",
        actionPath: "/accounts",
      },
    ];
  }, [totals, cats, netWorth]);

  // Keyboard navigation (list mode only)
  const askItems = useMemo<CmdItem[]>(
    () => questions.map((q) => ({ kind: "ask" as const, label: q.label, Icon: I.Sparkle, question: q })),
    [questions]
  );
  const all = useMemo(() => [...askItems, ...NAV_ITEMS, ...ACT_ITEMS], [askItems]);
  const filtered = useMemo(() => {
    if (!q.trim()) return all;
    const s = q.toLowerCase();
    // When searching, only show nav/act items (not ask items)
    return [...NAV_ITEMS, ...ACT_ITEMS].filter((x) => x.label.toLowerCase().includes(s));
  }, [q, all]);

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
    if (item.kind === "ask" && item.question) {
      setActiveQ(item.question);
      setMode("answer");
      return;
    }
    if (item.path) { navigate(item.path); onClose(); }
    else { onClose(); }
  };

  if (!open) return null;

  const askF = filtered.filter((x) => x.kind === "ask");
  const navsF = filtered.filter((x) => x.kind === "nav");
  const actsF = filtered.filter((x) => x.kind === "act");

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
        {mode === "answer" && activeQ ? (
          // ── Answer mode ──────────────────────────────────────────────────
          <>
            <div className="cmdk-input" style={{ borderBottom: "1px solid var(--hairline)" }}>
              <I.Sparkle style={{ color: "var(--accent)", width: 16, height: 16, flexShrink: 0 }} />
              <span style={{ flex: 1, fontSize: 14, color: "var(--ink-mute)" }}>{activeQ.label}</span>
              <button className="btn sm ghost" onClick={() => setMode("list")} style={{ fontSize: 12 }}>
                ← Back
              </button>
            </div>
            <div style={{ padding: "20px 24px" }}>
              <p style={{ marginBottom: 16, fontSize: 14, lineHeight: 1.6 }}>{activeQ.prose}</p>
              <AskViz question={activeQ} />
              {activeQ.actionLabel && activeQ.actionPath && (
                <div style={{ marginTop: 16 }}>
                  <button className="btn primary sm" onClick={() => { navigate(activeQ.actionPath!); onClose(); }}>
                    {activeQ.actionLabel}
                  </button>
                </div>
              )}
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
                placeholder="Search, jump to page, or type a command…"
                aria-label="Command palette input"
              />
              <span className="kbd" style={{ fontFamily: "var(--mono)" }}>esc</span>
            </div>
            <div className="cmdk-list" role="listbox">
              {filtered.length === 0 && (
                <div className="cmdk-item" style={{ color: "var(--ink-mute)" }}>
                  No results for "{q}"
                </div>
              )}
              {/* Ask the agent section */}
              {askF.length > 0 && !q.trim() && (
                <>
                  <div className="cmdk-section">Ask the agent</div>
                  {renderItems(askF)}
                </>
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
