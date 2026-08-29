import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useFocusParam } from "../api/hooks/useFocusParam";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { useBudgetEnvelopes, useBudgetHistory, useSetBudget, useGoals, useContributeToGoal, useMemberBudgetEnvelopes } from "../api/hooks/budget";
import { useHold, useSetHold } from "../api/hooks/budgetHolds";
import { useBudgetTransfers, useTransferBudget } from "../api/hooks/budgetTransfers";
import { useHouseholdMembers } from "../api/hooks/household";
import { useMonthTotals } from "../api/hooks/reports";
import { api, type BudgetEnvelope, type SpendingBreakdown } from "../api/openapiClient";
import { unwrap } from "../api/openapiClient";
import PlanNextMonthModal from "./PlanNextMonthModal";
import EmptyState from "../components/EmptyState";
import PageHeader from "../components/PageHeader";
import { money } from "../utils/format";
import { getBudgetReadiness } from "../utils/dataReadiness";
type SortKey = "group" | "stress" | "size" | "activity";

function envelopeStatus(env: BudgetEnvelope) {
  const transfer = (env as { transferCents?: number }).transferCents ?? 0;
  const available = env.budgetCents + env.carryoverCents + transfer;
  if (available <= 0 && env.budgetCents <= 0) return { label: "No budget set", tone: "warning" as const, severity: 2 };
  const pct = available > 0 ? (env.spentCents / available) * 100 : 100;
  if (env.spentCents > available) {
    return { label: `Over by ${money(env.spentCents - available)}`, tone: "negative" as const, severity: 3 };
  }
  if (pct > 90) return { label: "Almost used", tone: "warning" as const, severity: 2 };
  if (pct > 60) return { label: "Watch", tone: "accent" as const, severity: 1 };
  return { label: "Available", tone: "positive" as const, severity: 0 };
}

function BudgetInput({ envelope, onClose }: { envelope: BudgetEnvelope; onClose: () => void }) {
  const setBudget = useSetBudget();
  const [value, setValue] = useState(envelope.budgetCents > 0 ? String(Math.round(envelope.budgetCents / 100)) : "");

  const save = async () => {
    const amountCents = Math.round(Number(value || 0) * 100);
    try {
      await setBudget.mutateAsync({ categoryId: envelope.categoryId, amountCents });
      toast.success("Budget saved", { description: `${envelope.categoryLabel} · ${money(amountCents)}` });
      onClose();
    } catch {
      toast.error("Failed to save budget");
    }
  };

  return (
    <div className="card tight" style={{ marginTop: 12, padding: 16 }}>
      <div className="eyebrow"><span className="dot" />Adjust monthly budget</div>
      <div className="row row-sm" style={{ marginTop: 10, alignItems: "center", flexWrap: "wrap" }}>
        <input
          className="control"
          type="number"
          min="0"
          step="10"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void save();
            if (e.key === "Escape") onClose();
          }}
          aria-label={`Budget amount for ${envelope.categoryLabel}`}
          style={{ maxWidth: 180 }}
        />
        <button className="btn primary sm" type="button" onClick={() => void save()}>Save</button>
        <button className="btn ghost sm" type="button" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}

function EnvelopeCard({ env, editing, onEdit, donor, memberShareCents, memberName, onCover }: { env: BudgetEnvelope; editing: boolean; onEdit: () => void; donor: BudgetEnvelope | null; memberShareCents?: number; memberName?: string; onCover?: (from: BudgetEnvelope, to: BudgetEnvelope, amount: number) => void }) {
  const status = envelopeStatus(env);
  const transfer = (env as { transferCents?: number }).transferCents ?? 0;
  const available = env.budgetCents + env.carryoverCents + transfer;
  const remaining = available - env.spentCents;
  const pct = available > 0 ? Math.min(100, (env.spentCents / available) * 100) : 0;
  const toneClass = status.tone === "negative" ? "negative" : status.tone === "warning" ? "warning" : status.tone === "positive" ? "positive" : "accent";
  const daysLeft = Math.max(1, new Date(new Date().getFullYear(), new Date().getMonth() + 1, 0).getDate() - new Date().getDate());
  const perDay = remaining > 0 ? Math.round(remaining / daysLeft) : 0;

  return (
    <div
      className="card"
      style={{
        padding: 22,
        borderColor: status.tone === "negative" ? "var(--negative)" : status.tone === "warning" ? "var(--warning)" : "var(--line)",
        background: status.tone === "negative" ? "var(--negative-2)" : status.tone === "warning" ? "var(--warning-2)" : "var(--surface)",
      }}
    >
      <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
        <div>
          <div className="row row-sm" style={{ alignItems: "center", marginBottom: 8 }}>
            <span className="cswatch" style={{ background: env.categoryColor || "var(--accent)" }} />
            <strong>{env.categoryLabel}</strong>
            <span className="muted" style={{ fontSize: 12 }}>{env.txnCount} txn{env.txnCount === 1 ? "" : "s"}</span>
          </div>
          <div className="figure money" style={{ fontSize: 34, lineHeight: 1, color: remaining < 0 ? "var(--negative)" : "var(--ink)" }}>
            {money(Math.abs(remaining))}
          </div>
          <div className="muted" style={{ fontSize: 12.5, marginTop: 6 }}>{remaining < 0 ? "over budget" : "left to spend"}</div>
        </div>
        <span className={`chip ${toneClass}${status.label.includes("$") ? " blurable" : ""}`}>{status.label}</span>
      </div>

      <div className="goal-bar" style={{ marginTop: 16, height: 7 }}>
        <span
          style={{
            width: `${pct}%`,
            background: status.tone === "negative" ? "var(--negative)" : status.tone === "warning" ? "var(--warning)" : env.categoryColor || "var(--accent)",
            boxShadow: status.tone === "negative" ? "0 0 12px var(--negative-2)" : status.tone === "warning" ? "0 0 12px var(--warning-2)" : `0 0 12px ${env.categoryColor || "var(--accent-3)"}`,
          }}
        />
      </div>

      <div className="hero-meta" style={{ justifyContent: "space-between", marginTop: 10 }}>
        <span className="money">{money(env.spentCents)} spent</span>
        <span className="money">of {money(available)}</span>
      </div>

      {memberShareCents !== undefined && (
        <div className="budget-member-share">
          <span className="cswatch" style={{ background: "var(--accent)" }} />
          <span className="money">{money(memberShareCents)}</span>
          <span className="muted">{memberName}&apos;s share</span>
        </div>
      )}

      {env.carryoverCents !== 0 && (
        <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid var(--hairline)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span className="muted" style={{ fontSize: 12 }}>Carried from last month</span>
          <span className="money" style={{ fontSize: 12.5, color: env.carryoverCents > 0 ? "var(--positive)" : "var(--negative)" }}>
            {env.carryoverCents > 0 ? "+" : ""}{money(env.carryoverCents)}
          </span>
        </div>
      )}

      {transfer !== 0 && (
        <div style={{ marginTop: 8, display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span className="muted" style={{ fontSize: 12 }}>{transfer > 0 ? "Cover received" : "Cover sent"}</span>
          <span className="money" style={{ fontSize: 12.5, color: transfer > 0 ? "var(--positive)" : "var(--negative)" }}>
            {transfer > 0 ? "+" : ""}{money(transfer)}
          </span>
        </div>
      )}

      {status.tone === "negative" && (
        <button
          className="btn outline sm"
          type="button"
          style={{ marginTop: 14, width: "100%" }}
          onClick={() => {
            if (!donor) {
              toast("No envelope has spare room to cover this right now.");
              return;
            }
            const donorTransfer = (donor as { transferCents?: number }).transferCents ?? 0;
            const donorRemaining = donor.budgetCents + donor.carryoverCents + donorTransfer - donor.spentCents;
            const overspend = env.spentCents - available;
            if (onCover && overspend > 0 && donorRemaining >= overspend) {
              onCover(donor, env, overspend);
            } else {
              toast(`${donor.categoryLabel} has ${money(donorRemaining)} unspent — often the best donor.`, {
                description: overspend > 0 ? `Needs ${money(overspend)} to cover.` : "Adjust each envelope's budget below to move the amount over.",
              });
            }
          }}
        >
          Cover from another envelope
        </button>
      )}

      {status.tone === "warning" && remaining > 0 && (
        <div className="budget-pace-note">
          About <span className="money strong">{money(perDay)}</span>/day left to stay under.
        </div>
      )}

      <div className="row row-sm" style={{ marginTop: 14 }}>
        <button className="btn ghost sm" type="button" onClick={onEdit}>{editing ? "Editing…" : env.budgetCents > 0 ? "Adjust budget" : "Set budget"}</button>
      </div>
    </div>
  );
}

export default function Budget() {
  const navigate = useNavigate();
  const { data: envelopes = [], isLoading, error, refetch } = useBudgetEnvelopes();
  const { data: history = [] } = useBudgetHistory(5);
  const { data: totals } = useMonthTotals();
  const { data: goals = [] } = useGoals();
  const contribute = useContributeToGoal();
  const { data: breakdown } = useQuery<SpendingBreakdown>({
    queryKey: ["spending-breakdown"],
    queryFn: async () => {
      return unwrap(api.getSpendingBreakdown());
    },
    staleTime: 60_000,
  });
  const [sort, setSort] = useState<SortKey>("group");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showPlan, setShowPlan] = useState(false);
  const [showAllAttention, setShowAllAttention] = useState(false);
  // Which household member's share of the spend to overlay, or null for the
  // whole household. The budgets stay household-level either way — this only
  // scopes the "spent" side.
  const [scopeMemberId, setScopeMemberId] = useState<string | null>(null);
  const { data: members = [] } = useHouseholdMembers();
  const { data: memberEnvelopes = [] } = useMemberBudgetEnvelopes(scopeMemberId);
  // A member's share of spend, by category, for the overlay. Empty for the
  // household view.
  const memberSpendById = useMemo(() => {
    const map = new Map<string, number>();
    for (const env of memberEnvelopes) map.set(env.categoryId, env.memberSpentCents);
    return map;
  }, [memberEnvelopes]);
  const pendingScrollRef = useRef<string | null>(null);

  const now = new Date();
  const totalDays = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();
  const today = now.getDate();
  const monthLabel = now.toLocaleDateString("en-US", { month: "long", year: "numeric" });
  const monthPct = Math.round((today / totalDays) * 100);
  const currentMonth = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  const nextMonth = now.getMonth() === 11 ? `${now.getFullYear() + 1}-01` : `${now.getFullYear()}-${String(now.getMonth() + 2).padStart(2, "0")}`;
  const nextMonthLabel = new Date(Number(nextMonth.slice(0, 4)), Number(nextMonth.slice(5, 7)) - 1, 1).toLocaleDateString("en-US", { month: "long", year: "numeric" });
  const { data: hold } = useHold(currentMonth);
  const setHold = useSetHold();
  const [holdInput, setHoldInput] = useState("");
  const holdCents = hold?.amountCents ?? 0;
  // Sync input when hold loads (once) — keep user's edits intact.
  useEffect(() => {
    if (hold && holdInput === "") {
      setHoldInput(hold.amountCents > 0 ? String(Math.round(hold.amountCents / 100)) : "");
    }
  }, [hold, holdInput]);

  // Cover ledger — auditable per-month transfers
  const { data: transfers = [] } = useBudgetTransfers(currentMonth);
  const transferBudget = useTransferBudget();
  const [coverFrom, setCoverFrom] = useState<string>("");
  const [coverTo, setCoverTo] = useState<string>("");
  const [coverAmount, setCoverAmount] = useState<string>("");
  const [coverNote, setCoverNote] = useState<string>("");

  const handleCover = useCallback(
    async (from: BudgetEnvelope | string, to: BudgetEnvelope | string, amountCents: number, note?: string) => {
      const fromId = typeof from === "string" ? from : from.categoryId;
      const toId = typeof to === "string" ? to : to.categoryId;
      const amt = Math.round(amountCents);
      if (!fromId || !toId) {
        toast.error("Pick both a donor and a recipient");
        return;
      }
      if (fromId === toId) {
        toast.error("Donor and recipient must differ");
        return;
      }
      if (amt <= 0) {
        toast.error("Amount must be greater than $0");
        return;
      }
      try {
        await transferBudget.mutateAsync({
          fromCategory: fromId,
          toCategory: toId,
          amountCents: amt,
          month: currentMonth,
          note: note ?? coverNote ?? null,
        });
        toast.success("Cover moved", {
          description: `${money(amt)} from ${(typeof from === "string" ? envelopes.find((e) => e.categoryId === fromId)?.categoryLabel : from.categoryLabel) ?? fromId} → ${(typeof to === "string" ? envelopes.find((e) => e.categoryId === toId)?.categoryLabel : to.categoryLabel) ?? toId}`,
        });
        setCoverAmount("");
        setCoverNote("");
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        toast.error("Could not move cover", { description: msg });
      }
    },
    [transferBudget, currentMonth, coverNote, envelopes],
  );

  const handleCoverFromCard = useCallback(
    (donor: BudgetEnvelope, recipient: BudgetEnvelope, amountCents: number) => {
      setCoverFrom(donor.categoryId);
      setCoverTo(recipient.categoryId);
      setCoverAmount(String(Math.round(amountCents / 100)));
      // Scroll cover ledger into view
      document.getElementById("budget-cover-ledger")?.scrollIntoView({ block: "center" });
      // Auto-execute the exact overspend if the donor has spare (the button's pre-check already did)
      void handleCover(donor, recipient, amountCents, "cover overspend");
    },
    [handleCover],
  );

  const sorted = useMemo(() => [...envelopes].sort((a, b) => {
    if (sort === "stress") return envelopeStatus(b).severity - envelopeStatus(a).severity || b.spentCents - a.spentCents;
    if (sort === "size") return b.budgetCents - a.budgetCents;
    if (sort === "activity") return b.txnCount - a.txnCount;
    return (a.groupLabel || "").localeCompare(b.groupLabel || "") || a.categoryLabel.localeCompare(b.categoryLabel);
  }), [envelopes, sort]);



  const totalBudget = sorted.reduce((sum, env) => sum + env.budgetCents, 0);
  const totalCarryover = sorted.reduce((sum, env) => sum + env.carryoverCents, 0);
  const totalTransfer = sorted.reduce((sum, env) => sum + ((env as { transferCents?: number }).transferCents ?? 0), 0);
  const totalAvailable = totalBudget + totalCarryover + totalTransfer;
  const totalSpent = sorted.reduce((sum, env) => sum + env.spentCents, 0);
  const fixedSpent = Math.min(totalSpent, breakdown?.fixedCents ?? 0);
  const variableSpent = Math.max(0, totalSpent - fixedSpent);
  const variableProjection = today > 0 ? Math.round((variableSpent / today) * totalDays) : 0;
  const projectedEom = fixedSpent + variableProjection;
  const estimateSpread = today < 10 ? 0.2 : today < 21 ? 0.12 : 0.07;
  const estimateLow = Math.max(totalSpent, Math.round(projectedEom * (1 - estimateSpread)));
  const estimateHigh = Math.max(estimateLow, Math.round(projectedEom * (1 + estimateSpread)));
  const estimateConfidence = today < 10 ? "Early estimate" : today < 21 ? "Developing estimate" : "Higher-confidence estimate";
  const fundedEnvelopeCount = sorted.filter((env) => env.budgetCents > 0 || env.carryoverCents !== 0).length;
  const transactionCount = sorted.reduce((sum, env) => sum + env.txnCount, 0);
  const readiness = getBudgetReadiness({
    envelopeCount: sorted.length,
    fundedEnvelopeCount,
    transactionCount,
    spentCents: totalSpent,
    dayOfMonth: today,
  });
  const remaining = totalAvailable - totalSpent;
  const toBudget = (totals?.incomeCents ?? 0) - totalBudget - holdCents;
  const unbudgeted = sorted.filter((env) => env.budgetCents <= 0 && env.spentCents <= 0 && env.carryoverCents === 0);
  // Unbudgeted categories aren't "in trouble" (severity>=2 from "No budget
  // set" is really "unconfigured") — they get their own section below instead
  // of also cluttering "Needs a glance".
  const attention = sorted.filter((env) => envelopeStatus(env).severity >= 2 && !unbudgeted.includes(env));
  const attentionIds = new Set(attention.map((env) => env.categoryId));
  const visibleAttention = showAllAttention ? attention : attention.slice(0, 3);
  const regularEnvelopes = sorted.filter((env) => !unbudgeted.includes(env) && !attentionIds.has(env.categoryId));
  const grouped = Object.entries(regularEnvelopes.reduce<Record<string, BudgetEnvelope[]>>((acc, env) => {
    const key = sort === "group" ? env.groupLabel || "Other" : "All envelopes";
    acc[key] ||= [];
    acc[key].push(env);
    return acc;
  }, {}));

  const insight = readiness === "estimated"
    ? "Your plan is set. FinSight will estimate the month after it observes at least 10 transactions or a week of spending."
    : attention.length > 0
      ? `${attention.length} envelope${attention.length === 1 ? "" : "s"} need attention. ${projectedEom > totalBudget ? "The current estimate may finish above plan." : "The rest of the month still fits the plan."}`
      : projectedEom > totalBudget
        ? "The current estimate may finish above plan even though no single category is over its limit yet."
        : "Spending is within the plan based on the activity recorded so far.";

  const totalTagged = breakdown ? breakdown.fixedCents + breakdown.investmentsCents + breakdown.savingsCents + breakdown.guiltFreeCents + breakdown.untaggedCents : 0;

  // Deep-link support: ?focusCategory=<id-or-label> opens that envelope's
  // editor, matching the focus idiom used by Accounts, Goals and Recurring.
  const handleFocusCategory = useCallback(
    (raw: string) => {
      if (isLoading) return false;
      const match = envelopes.find(
        (env) => env.categoryId === raw || env.categoryLabel.toLowerCase() === raw.toLowerCase(),
      );
      const id = match?.categoryId ?? null;
      if (id && !editingId) {
        setEditingId(id);
        pendingScrollRef.current = id;
      }
    },
    [envelopes, editingId, isLoading],
  );
  useFocusParam("focusCategory", handleFocusCategory);

  // Runs after every render until the focused envelope is on screen: the row
  // does not exist yet on the render that sets `editingId`.
  useEffect(() => {
    const target = pendingScrollRef.current;
    if (!target) return;
    const el = document.querySelector(`[data-envelope-id="${CSS.escape(target)}"]`);
    if (!el) return;
    pendingScrollRef.current = null;
    // jsdom and older webviews do not implement scrollIntoView.
    el.scrollIntoView?.({ block: "center" });
  });

  const donorFor = (categoryId: string): BudgetEnvelope | null => {
    const availableFor = (env: BudgetEnvelope) =>
      env.budgetCents + env.carryoverCents + ((env as { transferCents?: number }).transferCents ?? 0) - env.spentCents;
    const candidates = sorted.filter((env) => env.categoryId !== categoryId && availableFor(env) > 0);
    if (candidates.length === 0) return null;
    return candidates.reduce((best, env) => (availableFor(env) > availableFor(best) ? env : best));
  };

  // Only manual (non-account-linked) goals accept recorded progress: a linked
  // goal's balance comes from its account, so a manual entry double-counts.
  const parkableGoal = goals.find((goal) => !goal.accountId) ?? null;

  const handleRecordGoalProgress = async () => {
    const firstGoal = parkableGoal;
    if (!firstGoal) {
      toast("Create a manual goal before recording this allocation.", {
        description: "Account-linked goals update from their connected balance.",
      });
      return;
    }
    if (toBudget <= 0) {
      toast("There is no unassigned income to record.");
      return;
    }
    try {
      await contribute.mutateAsync({
        id: firstGoal.id,
        amountCents: toBudget,
        note: "Recorded unassigned budget toward goal",
        source: "sweep",
      });
      toast.success(`Recorded ${money(toBudget)} toward ${firstGoal.name}`, {
        description: "This updates FinSight only. No money was moved.",
        action: {
          label: "Undo",
          onClick: () => {
            void contribute.mutateAsync({
              id: firstGoal.id,
              amountCents: -toBudget,
              note: "Undid recorded unassigned budget",
              source: "undo",
            }).then(
              () => toast.success("Recorded progress removed"),
              () => toast.error("Could not undo the recorded progress"),
            );
          },
        },
      });
    } catch {
      toast.error("Could not record goal progress", {
        description: "Your budget and bank balances were not changed. Try again.",
      });
    }
  };


  if (isLoading) {
    // Mirrors the real grid so the page does not collapse to one line and then
    // snap back into three columns once data lands.
    return (
      <div className="budget-loading" aria-live="polite" aria-busy="true">
        <span className="sr-only">Loading budget…</span>
        <div className="skeleton heading" style={{ width: 220 }} />
        <div className="budget-grid" aria-hidden="true">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="card budget-skel-card">
              <div className="skeleton text" style={{ width: "55%" }} />
              <div className="skeleton" style={{ height: 34, width: "45%", margin: "10px 0 8px" }} />
              <div className="skeleton text" style={{ width: "35%" }} />
              <div className="skeleton" style={{ height: 6, marginTop: 16 }} />
            </div>
          ))}
        </div>
      </div>
    );
  }
  if (error && envelopes.length === 0) {
    return (
      <div className="stub route-load-problem" role="alert">
        <div className="card">
          <h1>Budget could not load</h1>
          <p className="muted">Your plan is unchanged. Check the connection, then try loading it again.</p>
          <button className="btn primary" type="button" onClick={() => void refetch()}>Try again</button>
        </div>
      </div>
    );
  }

  if (envelopes.length === 0) {
    return (
      <div className="screen screen-budget">
        <PageHeader
          eyebrow={<>Budget · {monthLabel}</>}
          title="Build your first monthly plan."
        />
        <EmptyState
          title="Start with categories you actually spend in"
          description="Create a simple plan or import transaction history first. FinSight will not estimate pace or project the month until there is enough activity to support it."
          details={
            <ul className="empty-unlocks">
              <li>Set practical limits for the month</li>
              <li>See what remains after recorded spending</li>
              <li>Unlock end-of-month estimates after enough activity</li>
            </ul>
          }
          actions={<>
            <button className="btn primary" type="button" onClick={() => setShowPlan(true)}>Create first budget</button>
            <button className="btn outline" type="button" onClick={() => navigate("/accounts")}>Import transactions</button>
          </>}
        />
        {showPlan && <PlanNextMonthModal onClose={() => setShowPlan(false)} />}
      </div>
    );
  }

  return (
    <div className="screen screen-budget">
      <PageHeader
        eyebrow={<>Budget · {monthLabel} · day {today} of {totalDays}</>}
        title="Where the plan stands today."
        actions={<div className="row row-sm wrap" style={{ justifyContent: "flex-end" }}>
          {members.length > 0 && (
            <div className="budget-scope" role="group" aria-label="Whose spending to show">
              <button
                type="button"
                className={`budget-scope-btn${scopeMemberId === null ? " is-on" : ""}`}
                aria-pressed={scopeMemberId === null}
                onClick={() => setScopeMemberId(null)}
              >
                Household
              </button>
              {members.map((m) => (
                <button
                  key={m.id}
                  type="button"
                  className={`budget-scope-btn${scopeMemberId === m.id ? " is-on" : ""}`}
                  aria-pressed={scopeMemberId === m.id}
                  onClick={() => setScopeMemberId(m.id)}
                >
                  {m.color && <span className="cswatch" style={{ background: m.color }} />}
                  {m.name}
                </button>
              ))}
            </div>
          )}
          <button className="btn primary" type="button" onClick={() => setShowPlan(true)}>Plan next month</button>
        </div>}
      />
      {scopeMemberId !== null && (
        <p className="muted budget-scope-note">
          Showing {members.find((m) => m.id === scopeMemberId)?.name}&apos;s share of the spend against
          each shared budget — the targets are still the household&apos;s.
        </p>
      )}

      {readiness === "unavailable" && (
        <EmptyState
          compact
          title="Set the first budget amounts"
          description="Your categories are here, but no money has been assigned yet. FinSight will wait to show pace and projections until the plan has real activity behind it."
          actions={<button className="btn primary sm" type="button" onClick={() => setShowPlan(true)}>Plan categories</button>}
        />
      )}

      {readiness !== "unavailable" && (
      <div className="card budget-overview" style={{ padding: 28 }}>
        <div className="budget-hero-grid">
          <div>
            <div className="eyebrow"><span className="dot" />Month progress</div>
            <div className="hero-num">
              <div className="figure money" style={{ fontSize: 56, lineHeight: 1, color: remaining < 0 ? "var(--negative)" : "var(--accent)" }}>{money(Math.abs(remaining))}</div>
              <div className="muted">{remaining < 0 ? "over the current plan" : "left to spend"}</div>
            </div>
            <div className="budget-progress-track" style={{ position: "relative", height: 10, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginTop: 4 }}>
              <div className="budget-progress-time" style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: `${monthPct}%`, background: "var(--ink-faint)", opacity: 0.4, borderRadius: 999 }} title="Time elapsed" />
              <div className="budget-progress-spend" style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: `${totalAvailable > 0 ? Math.min(100, (totalSpent / totalAvailable) * 100) : 0}%`, background: "var(--accent)", borderRadius: 999, boxShadow: "0 0 12px var(--accent-3)" }} title="Spent" />
            </div>
            <div className="hero-meta" style={{ marginTop: 10 }}>
              <span>{monthPct}% through {now.toLocaleString("en-US", { month: "long" })}</span>
              <span>{totalBudget > 0 ? Math.round((totalSpent / totalBudget) * 100) : 0}% spent</span>
              <span>{totalDays - today} days left</span>
            </div>
          </div>
          <div className="budget-grid">
            <div className="stat"><div className="label">Budgeted</div><div className="value money">{money(totalBudget)}</div><div className="sub">Across {sorted.length} envelopes</div></div>
            <div className="stat"><div className="label">Spent so far</div><div className="value money">{money(totalSpent)}</div><div className="sub">{readiness === "reliable" ? <><span className="blurable">{today > 0 ? money(Math.round(totalSpent / today)) : money(0)}</span>/day pace</> : <>{transactionCount} recorded transaction{transactionCount === 1 ? "" : "s"}</>}</div></div>
            <div className="stat budget-forecast">
              <div className="label">Likely month-end range</div>
              {readiness === "reliable" ? (
                <>
                  <div className="value money">{money(estimateLow)}–{money(estimateHigh)}</div>
                  <div className="sub">
                    <span className="chip warning">{estimateConfidence}</span>
                    {estimateHigh > totalAvailable
                      ? <span className="budget-forecast-risk">Could finish up to <span className="money">{money(estimateHigh - totalAvailable)}</span> above the current plan.</span>
                      : <span>The current range fits the amount available.</span>}
                  </div>
                  <details className="budget-estimate-details">
                    <summary>How this range is calculated</summary>
                    <p>
                      FinSight keeps {money(fixedSpent)} of fixed costs already recorded, then projects the pace of
                      {money(variableSpent)} in variable spending. The range is wider earlier in the month.
                    </p>
                  </details>
                </>
              ) : (
                <><div className="value">Not ready</div><div className="sub">FinSight needs a week or 10 transactions before projecting the month.</div></>
              )}
            </div>
          </div>
        </div>
        <p className="muted" style={{ marginTop: 18, marginBottom: 0, maxWidth: 900 }}>{insight}</p>
      </div>

      )}

      {readiness !== "unavailable" && (
      <div className="card tight budget-allocation-card" style={{ marginTop: 16, padding: 18, display: "grid", gridTemplateColumns: "1.7fr auto", gap: 16, alignItems: "center" }}>
        <div>
          <div className="eyebrow"><span className="dot" />{toBudget < 0 ? "Over-assigned" : "Unassigned income"}</div>
          <div className="row row-sm wrap" style={{ alignItems: "baseline", marginTop: 8 }}>
            <div className="figure money" style={{ fontSize: 32, color: toBudget >= 0 ? "var(--accent)" : "var(--negative)" }}>{money(Math.abs(toBudget))}</div>
            <div className="muted">{toBudget < 0
              ? <>more assigned than the <span className="money">{money(totals?.incomeCents ?? 0)}</span> income recorded this month · <span className="money">{money(totalBudget)}</span> assigned</>
              : <>available to plan from <span className="money">{money(totals?.incomeCents ?? 0)}</span> income · <span className="money">{money(totalBudget)}</span> assigned</>
            }</div>
          </div>
        </div>
        {toBudget > 0
          ? <div className="budget-allocation-action"><span>This records progress in FinSight. It does not move money.</span><div className="row row-sm wrap"><button className="btn outline sm" type="button" onClick={() => navigate("/goals")}>Choose a goal</button><button className="btn sm" type="button" disabled={contribute.isPending} onClick={() => void handleRecordGoalProgress()}>Record toward {parkableGoal?.name ?? "a goal"}</button></div></div>
          : <span className="muted">{toBudget < 0
            ? <>Reduce planned budgets by <span className="money">{money(Math.abs(toBudget))}</span> or record more income.</>
            : "No unassigned income to move."
          }</span>}
      </div>

      )}
            {readiness !== "unavailable" && (
        <div className="card tight" style={{ marginTop: 16, padding: 18 }}>
          <div className="eyebrow"><span className="dot" />Hold for next month</div>
          <div className="row row-sm wrap" style={{ marginTop: 8, alignItems: "center", gap: 12, flexWrap: "wrap" }}>
            <span className="muted" style={{ fontSize: 12.5 }}>
              Park unassigned income for {nextMonthLabel}. Held money reduces this month&apos;s To Budget and will be available in {nextMonthLabel} as income-like.
            </span>
            {holdCents > 0 && <span className="money" style={{ fontSize: 13, color: "var(--accent)" }}>{money(holdCents)} held</span>}
          </div>
          <div className="row row-sm" style={{ marginTop: 12, alignItems: "center", flexWrap: "wrap", gap: 8 }}>
            <input
              className="control"
              type="text"
              inputMode="numeric"
              pattern="[0-9]*"
              placeholder="Amount $"
              value={holdInput}
              onChange={(e) => setHoldInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  const v = Math.round(Number(holdInput || 0) * 100);
                  if (v < 0) { toast.error("Hold must be $0 or more"); return; }
                  void setHold.mutateAsync({ month: currentMonth, amountCents: v }).then(
                    () => toast.success("Hold saved", { description: `${money(v)} held for ${nextMonthLabel}` }),
                    () => toast.error("Could not save hold"),
                  );
                }
                if (e.key === "Escape") setHoldInput(hold ? String(Math.round(hold.amountCents / 100)) : "");
              }}
              aria-label={`Hold amount for ${nextMonthLabel}`}
              style={{ maxWidth: 160 }}
            />
            <button
              className="btn primary sm"
              type="button"
              disabled={setHold.isPending}
              onClick={() => {
                const v = Math.round(Number(holdInput || 0) * 100);
                if (v < 0) { toast.error("Hold must be $0 or more"); return; }
                void setHold.mutateAsync({ month: currentMonth, amountCents: v }).then(
                  () => toast.success("Hold saved", { description: `${money(v)} held for ${nextMonthLabel}` }),
                  () => toast.error("Could not save hold"),
                );
              }}
            >
              {setHold.isPending ? "Saving…" : "Save hold"}
            </button>
            {holdCents > 0 && (
              <button
                className="btn ghost sm"
                type="button"
                disabled={setHold.isPending}
                onClick={() => {
                  void setHold.mutateAsync({ month: currentMonth, amountCents: 0 }).then(
                    () => { setHoldInput(""); toast.success("Hold cleared"); },
                    () => toast.error("Could not clear hold"),
                  );
                }}
              >
                Clear
              </button>
            )}
          </div>
          {holdCents > 0 && (
            <p className="muted" style={{ marginTop: 8, fontSize: 12, marginBottom: 0 }}>
              {money(holdCents)} will be added to {nextMonthLabel}&apos;s available funds as income-like. To Budget this month is <span className="money">{money(toBudget)}</span> after the hold.
            </p>
          )}
        </div>
      )}

      {readiness !== "unavailable" && (
        <div id="budget-cover-ledger" className="card tight" style={{ marginTop: 16, padding: 18 }}>
          <div className="eyebrow"><span className="dot" />Cover ledger · {currentMonth}</div>
          <p className="muted" style={{ fontSize: 12.5, marginTop: 6, marginBottom: 0 }}>
            Move leftover money between envelopes to cover overspend.
          </p>

          {transfers.length > 0 ? (
            <div className="tbl-scroll" style={{ marginTop: 14 }}>
              <table className="tbl" style={{ fontSize: 13 }}>
                <thead>
                  <tr>
                    <th>From</th>
                    <th>To</th>
                    <th className="right">Amount</th>
                    <th>Note</th>
                    <th className="right">Date</th>
                  </tr>
                </thead>
                <tbody>
                  {transfers.map((t) => {
                    const fromLabel = t.fromCategory ? (envelopes.find((e) => e.categoryId === t.fromCategory)?.categoryLabel ?? t.fromCategory) : "To Budget";
                    const toLabel = t.toCategory ? (envelopes.find((e) => e.categoryId === t.toCategory)?.categoryLabel ?? t.toCategory) : "To Budget";
                    return (
                      <tr key={t.id}>
                        <td>{fromLabel}</td>
                        <td>{toLabel}</td>
                        <td className="right money">{money(t.amountCents)}</td>
                        <td className="muted" style={{ maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{t.note ?? "—"}</td>
                        <td className="right muted" style={{ fontSize: 11 }}>{new Date(t.createdAt).toLocaleString()}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : (
            <p className="muted" style={{ fontSize: 12.5, marginTop: 12, marginBottom: 0 }}>No covers this month. When you move money, it appears here for audit.</p>
          )}

          {sorted.length >= 2 ? (
            <div className="card" style={{ marginTop: 16, padding: 14, background: "var(--surface-2)" }}>
              <div className="eyebrow" style={{ marginBottom: 10 }}>Move cover</div>
              <div className="row row-sm wrap" style={{ gap: 8, alignItems: "flex-end" }}>
                <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: "1 1 140px" }}>
                  <span className="muted" style={{ fontSize: 11 }}>From</span>
                  <select className="control" value={coverFrom} onChange={(e) => setCoverFrom(e.target.value)}>
                    <option value="">Donor…</option>
                    {sorted.map((e) => (
                      <option key={e.categoryId} value={e.categoryId}>{e.categoryLabel}</option>
                    ))}
                  </select>
                </label>
                <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: "1 1 140px" }}>
                  <span className="muted" style={{ fontSize: 11 }}>To</span>
                  <select className="control" value={coverTo} onChange={(e) => setCoverTo(e.target.value)}>
                    <option value="">Recipient…</option>
                    {sorted.map((e) => (
                      <option key={e.categoryId} value={e.categoryId}>{e.categoryLabel}</option>
                    ))}
                  </select>
                </label>
              <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: "0 0 120px" }}>
                <span className="muted" style={{ fontSize: 11 }}>Amount $</span>
                <input
                  className="control"
                  type="text"
                  inputMode="numeric"
                  placeholder="0.00"
                  value={coverAmount}
                  onChange={(e) => setCoverAmount(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      const amt = Math.round(Number(coverAmount || 0) * 100);
                      void handleCover(coverFrom, coverTo, amt);
                    }
                  }}
                />
              </label>
              <label style={{ display: "flex", flexDirection: "column", gap: 4, flex: "1 1 160px" }}>
                <span className="muted" style={{ fontSize: 11 }}>Note</span>
                <input className="control" type="text" placeholder="cover" value={coverNote} onChange={(e) => setCoverNote(e.target.value)} />
              </label>
              <button
                className="btn primary sm"
                type="button"
                disabled={transferBudget.isPending || !coverFrom || !coverTo || !coverAmount}
                onClick={() => {
                  const amt = Math.round(Number(coverAmount || 0) * 100);
                  void handleCover(coverFrom, coverTo, amt);
                }}
              >
                {transferBudget.isPending ? "Moving…" : "Move"}
              </button>
            </div>
            {coverFrom && coverTo && (() => {
              const fromEnv = sorted.find((e) => e.categoryId === coverFrom);
              const toEnv = sorted.find((e) => e.categoryId === coverTo);
              if (!fromEnv || !toEnv) return null;
              const fromAvail = fromEnv!.budgetCents + fromEnv!.carryoverCents + ((fromEnv! as { transferCents?: number }).transferCents ?? 0) - fromEnv!.spentCents;
              const toAvail = toEnv!.budgetCents + toEnv!.carryoverCents + ((toEnv! as { transferCents?: number }).transferCents ?? 0) - toEnv!.spentCents;
              return (
                <p className="muted" style={{ fontSize: 11, marginTop: 8, marginBottom: 0 }}>
                  {fromEnv!.categoryLabel} has <span className="money">{money(fromAvail)}</span> spare ·{" "}
                  {toEnv!.categoryLabel} is <span className="money" style={{ color: toAvail < 0 ? "var(--negative)" : "var(--ink)" }}>{toAvail < 0 ? `${money(Math.abs(toAvail))} over` : `${money(toAvail)} left`}</span>.
                </p>
              );
            })()}
          </div>
          ) : (
            <p className="muted" style={{ fontSize: 11, marginTop: 16, marginBottom: 0 }}>
              Add at least two budgeted categories to use cover.
            </p>
          )}
        </div>
      )}

      {breakdown && totalTagged > 0 && <div className="card tight" style={{ marginTop: 16 }}><div className="eyebrow"><span className="dot" />Spending mix</div><div className="stream" style={{ marginTop: 10, height: 16, borderRadius: 6 }}><span style={{ width: `${(breakdown.fixedCents / totalTagged) * 100}%`, background: "var(--ink-mute)" }} /><span style={{ width: `${(breakdown.investmentsCents / totalTagged) * 100}%`, background: "var(--accent)" }} /><span style={{ width: `${(breakdown.savingsCents / totalTagged) * 100}%`, background: "var(--positive)" }} /><span style={{ width: `${(breakdown.guiltFreeCents / totalTagged) * 100}%`, background: "var(--c-dining)" }} /><span style={{ width: `${(breakdown.untaggedCents / totalTagged) * 100}%`, background: "var(--ink-faint)" }} /></div></div>}

      {attention.length > 0 && (
        <section className="section" id="budget-attention">
          <div className="day-hdr" style={{ marginBottom: 14 }}>
            <div>
              <div className="eyebrow"><span className="dot" />Needs attention · {attention.length}</div>
              <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Start with these categories.</h2>
            </div>
            {attention.length > 3 && (
              <button className="btn ghost sm" type="button" onClick={() => setShowAllAttention((show) => !show)}>
                {showAllAttention ? "Show fewer" : `Show ${attention.length - 3} more`}
              </button>
            )}
          </div>
          <div className="budget-grid">
            {visibleAttention.map((env) => (
              <div key={env.categoryId} data-envelope-id={env.categoryId}>
                <EnvelopeCard env={env} editing={editingId === env.categoryId} onEdit={() => setEditingId(env.categoryId)} donor={donorFor(env.categoryId)} memberShareCents={scopeMemberId !== null ? (memberSpendById.get(env.categoryId) ?? 0) : undefined} memberName={members.find((m) => m.id === scopeMemberId)?.name} onCover={handleCoverFromCard} />
                {editingId === env.categoryId && <BudgetInput envelope={env} onClose={() => setEditingId(null)} />}
              </div>
            ))}
          </div>
        </section>
      )}

      {grouped.length > 0 && <section className="section">
        <div className="day-hdr" style={{ marginBottom: 14 }}>
          <div>
            <div className="eyebrow"><span className="dot" />Everything else</div>
            <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Remaining envelopes.</h2>
          </div>
          <label className="budget-sort">
            <span>Sort</span>
            <select value={sort} onChange={(event) => setSort(event.target.value as SortKey)}>
              <option value="group">By group</option>
              <option value="stress">By stress</option>
              <option value="size">By size</option>
              <option value="activity">By activity</option>
            </select>
          </label>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 28 }}>{grouped.map(([label, items]) => {
          const groupSpent = items.reduce((sum, env) => sum + env.spentCents, 0);
          const groupBudget = items.reduce((sum, env) => sum + env.budgetCents, 0);
          return (
            <div key={label}>
              <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", marginBottom: 12 }}>
                <div className="eyebrow">{label}</div>
                {sort === "group" && <span className="muted mono blurable" style={{ fontSize: 12.5 }}>{money(groupSpent)} / {money(groupBudget)}</span>}
              </div>
              <div className="budget-grid">{items.map((env) => <div key={env.categoryId} data-envelope-id={env.categoryId}><EnvelopeCard env={env} editing={editingId === env.categoryId} onEdit={() => setEditingId(env.categoryId)} donor={donorFor(env.categoryId)} memberShareCents={scopeMemberId !== null ? (memberSpendById.get(env.categoryId) ?? 0) : undefined} memberName={members.find((m) => m.id === scopeMemberId)?.name} onCover={handleCoverFromCard} />{editingId === env.categoryId && <BudgetInput envelope={env} onClose={() => setEditingId(null)} />}</div>)}</div>
            </div>
          );
        })}</div>
      </section>}

      {unbudgeted.length > 0 && (
        <section className="section">
          <div className="day-hdr" style={{ marginBottom: 14 }}>
            <div>
              <div className="eyebrow"><span className="dot" />Not yet budgeted · {unbudgeted.length}</div>
              <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>Set limits for these categories.</h2>
            </div>
          </div>
          <div className="budget-grid">
            {unbudgeted.map((env) => (
              <div key={env.categoryId} data-envelope-id={env.categoryId} className="card tight" style={{ padding: 18, display: "flex", flexDirection: "column", gap: 10 }}>
                <div className="row row-sm" style={{ alignItems: "center" }}>
                  <span className="cswatch" style={{ background: env.categoryColor || "var(--accent)" }} />
                  <strong>{env.categoryLabel}</strong>
                </div>
                {editingId === env.categoryId ? (
                  <BudgetInput envelope={env} onClose={() => setEditingId(null)} />
                ) : (
                  <button className="btn outline sm" type="button" onClick={() => setEditingId(env.categoryId)}>Set budget</button>
                )}
              </div>
            ))}
          </div>
        </section>
      )}

      {history.length > 0 && (
        <details className="section budget-secondary">
          <summary>Spending history · last 5 months</summary>
          <div className="card flush">
            <div className="tbl-scroll">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Category</th>
                  {history[0]?.monthly.map((m) => <th key={m.month} className="right">{m.label}</th>)}
                  <th className="right">Your typical</th>
                </tr>
              </thead>
              <tbody>
                {history.map((row) => {
                  const typicalCents = Math.round(
                    row.monthly.reduce((sum, m) => sum + m.spentCents, 0) / Math.max(1, row.monthly.length),
                  );
                  return (
                    <tr key={row.categoryId}>
                      <td><span className="cswatch" style={{ background: row.color || "var(--accent)" }} /> {row.label}</td>
                      {row.monthly.map((m) => {
                        const over = m.budgetedCents > 0 && m.spentCents > m.budgetedCents;
                        return (
                          <td key={m.month} className="right">
                            <span className={`money ${over ? "neg" : ""}`}>{money(m.spentCents)}</span>
                            {m.budgetedCents > 0 && <span className="muted" style={{ fontSize: 11, display: "block" }}>of <span className="blurable">{money(m.budgetedCents)}</span></span>}
                          </td>
                        );
                      })}
                      <td className="right"><span className="money muted">{money(typicalCents)}</span></td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            </div>
          </div>
        </details>
      )}

      {showPlan && <PlanNextMonthModal onClose={() => setShowPlan(false)} />}
    </div>
  );
}
