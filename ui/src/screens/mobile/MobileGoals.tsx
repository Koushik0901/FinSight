import { useState, useMemo } from "react";
import { useGoals, useUpdateGoalMonthly } from "../../api/hooks/budget";
import { useAccounts } from "../../api/hooks/accounts";
import type { GoalDto } from "../../api/openapiClient";
import { money } from "../../utils/format";
import { formatCalendarDate } from "../../utils/date";
import { toast } from "sonner";
import * as I from "../../components/Icons";
import { MobileEmptyState } from "../../components/mobile/MobileEmptyState";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import GoalDrawer from "../../components/GoalDrawer";
import { SegmentedControl } from "../../components/mobile/SegmentedControl";
import Reveal from "../../components/Reveal";
const TYPE_LABELS: Record<string, string> = {
  "save-by-date": "Save by date",
  "build-balance": "Build balance",
  "debt-payoff": "Pay off debt",
  "spending-cap": "Spending cap",
  "sinking-fund": "Sinking fund",
};

function paceLabel(goal: GoalDto) {
  if (goal.goalType === "spending-cap") {
    if (goal.targetCents <= 0) return { label: "No cap set", tone: "warning" as const };
    const used = goal.currentCents / goal.targetCents;
    if (goal.currentCents > goal.targetCents) return { label: "Over cap", tone: "negative" as const };
    if (used > 0.9) return { label: "Near cap", tone: "warning" as const };
    return { label: "Within cap", tone: "positive" as const };
  }
  const remaining = goal.targetCents - goal.currentCents;
  if (remaining <= 0) return { label: "Funded", tone: "positive" as const };
  if (goal.monthlyCents <= 0) return { label: "Needs attention", tone: "warning" as const };
  return { label: "On track", tone: "accent" as const };
}

function monthsToGoal(goal: GoalDto) {
  const remaining = goal.targetCents - goal.currentCents;
  if (remaining <= 0) return 0;
  if (goal.monthlyCents <= 0) return Infinity;
  return Math.ceil(remaining / goal.monthlyCents);
}

function etaLabel(months: number) {
  if (!Number.isFinite(months)) return "—";
  const d = new Date(); d.setDate(1); d.setMonth(d.getMonth() + months);
  return d.toLocaleDateString("en-US", { month: "short", year: "numeric" });
}

export default function MobileGoals() {
  const { data: goals = [], isLoading } = useGoals();
  const { data: accounts = [] } = useAccounts();
  const [filter, setFilter] = useState<string>("all");
  const [detail, setDetail] = useState<GoalDto | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editGoal, setEditGoal] = useState<GoalDto | null>(null);
  const updateMonthly = useUpdateGoalMonthly();

  const accountById = useMemo(() => Object.fromEntries(accounts.map((a) => [a.id, a.name])), [accounts]);

  const filtered = useMemo(() => {
    if (filter === "all") return goals;
    return goals.filter((g) => g.goalType === filter);
  }, [goals, filter]);

  const detailProjection = useMemo(() => {
    if (!detail || detail.goalType === "spending-cap") return null;
    const months = monthsToGoal(detail);
    return { months, eta: etaLabel(months) };
  }, [detail]);

  if (isLoading) return <div className="stub"><span className="spinner" aria-hidden="true" /> Loading…</div>;

  if (goals.length === 0) {
    return (
      <div style={{ padding: 16 }}>
        <MobileEmptyState
          icon={<I.Goal width={28} height={28} />}
          title="No goals yet"
          description="Create a savings target, debt payoff plan, or spending cap. Set a monthly contribution and watch progress."
          primaryAction={<button className="btn primary" type="button" onClick={() => setDrawerOpen(true)}>Create goal</button>}
        />
        <GoalDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} goal={null} />
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      <SegmentedControl
        options={[
          { value: "all", label: "All" },
          { value: "save-by-date", label: "Save" },
          { value: "build-balance", label: "Build" },
          { value: "debt-payoff", label: "Debt" },
          { value: "spending-cap", label: "Caps" },
        ]}
        value={filter}
        onChange={setFilter}
        ariaLabel="Goal filter"
      />

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {filtered.map((goal) => {
          const pct = goal.targetCents > 0 ? Math.min(100, Math.round((goal.currentCents / goal.targetCents) * 100)) : 0;
          const pace = paceLabel(goal);
          const months = monthsToGoal(goal);
          const tone = pace.tone === "negative" ? "negative" : pace.tone === "warning" ? "warning" : pace.tone === "positive" ? "positive" : "accent";
          return (
            <button
              key={goal.id}
              type="button"
              onClick={() => setDetail(goal)}
              style={{
                textAlign: "left",
                padding: 16,
                borderRadius: 16,
                border: "1px solid var(--line)",
                background: "var(--surface)",
                display: "flex",
                flexDirection: "column",
                gap: 10,
                cursor: "pointer",
              }}
            >
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
                <span className="chip" style={{ fontSize: 11 }}>{TYPE_LABELS[goal.goalType] ?? goal.goalType}</span>
                <span className={`chip ${tone}`} style={{ fontSize: 11 }}>{pace.label}</span>
                {goal.targetDate ? <span style={{ fontSize: 11, color: "var(--ink-faint)" }}>due {formatCalendarDate(goal.targetDate, { month: "short", year: "numeric" })}</span> : null}
              </div>
              <h2 style={{ margin: 0, fontSize: 18, fontWeight: 700, letterSpacing: "-0.02em", lineHeight: 1.2, color: "var(--ink)" }}>{goal.name}</h2>
              <div style={{ fontSize: 12, color: "var(--ink-mute)" }}>
                {goal.goalType === "debt-payoff" ? `Paying ${money(goal.monthlyCents)}/mo` : goal.goalType === "spending-cap" ? `Cap ${money(goal.targetCents)}/mo` : `${money(goal.monthlyCents)}/mo`}
                {goal.accountId ? ` · ${accountById[goal.accountId] ?? ""}` : ""}
              </div>

              <div style={{ marginTop: 2 }}>
                <div style={{ fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>{goal.goalType === "spending-cap" ? "This month" : "Progress"}</div>
                <Reveal>
                  <div style={{ height: 8, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginTop: 6 }}>
                    <div className="plot-grow-x" style={{ width: `${pct}%`, height: "100%", background: goal.goalType === "spending-cap" && goal.currentCents > goal.targetCents ? "var(--negative)" : "var(--accent)", borderRadius: 999 }} />
                  </div>
                </Reveal>
                <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6, fontSize: 13 }}>
                  <span className="money" style={{ fontWeight: 650 }}>{money(goal.currentCents)}</span>
                  <span className="money" style={{ color: "var(--ink-mute)" }}>of {money(goal.targetCents)}</span>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", marginTop: 4, fontSize: 11, color: "var(--ink-faint)" }}>
                  <span>{pct}%</span>
                  <span>{Number.isFinite(months) ? `ETA ${etaLabel(months)}` : "Paused"}</span>
                </div>
              </div>
            </button>
          );
        })}
      </div>

      <button type="button" className="btn primary" style={{ width: "100%", minHeight: 44, marginTop: 8 }} onClick={() => { setEditGoal(null); setDrawerOpen(true); }}>
        <I.Plus width={14} height={14} /> New goal
      </button>

      {/* Detail sheet with projector */}
      <BottomSheet open={!!detail} onClose={() => setDetail(null)} title={detail?.name ?? "Goal"}>
        {detail ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {(() => {
              const pct = detail.targetCents > 0 ? Math.min(100, Math.round((detail.currentCents / detail.targetCents) * 100)) : 0;
              const pace = paceLabel(detail);
              return (
                <>
                  <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                    <span className="chip">{TYPE_LABELS[detail.goalType] ?? detail.goalType}</span>
                    <span className={`chip ${pace.tone}`}>{pace.label}</span>
                  </div>

                  <div style={{ padding: 14, border: "1px solid var(--line)", borderRadius: 14, background: "var(--surface)" }}>
                    <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>Target</div>
                    <div className="money" style={{ fontSize: 24, fontWeight: 750, letterSpacing: "-0.02em", marginTop: 4 }}>{money(detail.targetCents)}</div>
                    <div style={{ fontSize: 12, color: "var(--ink-mute)", marginTop: 4 }}>{money(detail.currentCents)} saved · {pct}% · {detailProjection?.eta ? `ETA ${detailProjection.eta}` : detail.targetDate ? `due ${formatCalendarDate(detail.targetDate, { month: "short", year: "numeric" })}` : ""}</div>
                    <Reveal>
                      <div style={{ height: 8, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginTop: 10 }}>
                        <div className="plot-grow-x" style={{ width: `${pct}%`, height: "100%", background: "var(--accent)", borderRadius: 999 }} />
                      </div>
                    </Reveal>
                  </div>

                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
                    <div style={{ padding: 12, border: "1px solid var(--line)", borderRadius: 12, background: "var(--surface)" }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>Monthly</div>
                      <div className="money" style={{ fontWeight: 650, marginTop: 4 }}>{money(detail.monthlyCents)}</div>
                      <div style={{ fontSize: 11, color: "var(--ink-mute)", marginTop: 4 }}>{Number.isFinite(detailProjection?.months ?? 0) ? `${detailProjection?.months} mo left` : "Paused"}</div>
                    </div>
                    <div style={{ padding: 12, border: "1px solid var(--line)", borderRadius: 12, background: "var(--surface)" }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>Pace</div>
                      <div style={{ fontWeight: 650, marginTop: 4, fontSize: 13 }}>{pace.label}</div>
                      <div style={{ fontSize: 11, color: "var(--ink-mute)", marginTop: 4 }}>{detail.targetDate ? `Target ${formatCalendarDate(detail.targetDate, { month: "short", year: "numeric" })}` : "No deadline"}</div>
                    </div>
                  </div>

                  {/* Compound growth projector — only inside detail */}
                  {detail.goalType !== "spending-cap" && detail.goalType !== "debt-payoff" ? (
                    <div style={{ padding: 14, border: "1px solid var(--line)", borderRadius: 14, background: "var(--surface-2)" }}>
                      <div style={{ fontSize: 12, fontWeight: 650, color: "var(--ink)" }}>What if I change the monthly amount?</div>
                      <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
                        {[Math.max(0, detail.monthlyCents - 5000), detail.monthlyCents, detail.monthlyCents + 5000, detail.monthlyCents + 10000].map((amt) => {
                          const m = amt <= 0 ? Infinity : Math.ceil((detail.targetCents - detail.currentCents) / amt);
                          return (
                            <button
                              key={amt}
                              type="button"
                              onClick={async () => {
                                try { await updateMonthly.mutateAsync({ id: detail.id, monthlyCents: amt }); toast.success(`Monthly set to ${money(amt)}`); setDetail({ ...detail, monthlyCents: amt }); } catch { toast.error("Failed to update"); }
                              }}
                              style={{
                                flex: 1,
                                padding: "10px 6px",
                                borderRadius: 10,
                                border: amt === detail.monthlyCents ? "2px solid var(--accent)" : "1px solid var(--line)",
                                background: amt === detail.monthlyCents ? "var(--accent-2)" : "var(--surface)",
                                fontSize: 12,
                                fontWeight: 600,
                                textAlign: "center",
                              }}
                            >
                              <span className="money" style={{ display: "block", fontSize: 12 }}>{money(amt)}</span>
                              <span style={{ display: "block", fontSize: 10, color: "var(--ink-mute)", marginTop: 2 }}>{Number.isFinite(m) ? etaLabel(m) : "—"}</span>
                            </button>
                          );
                        })}
                      </div>
                      <div style={{ fontSize: 11, color: "var(--ink-faint)", marginTop: 8 }}>Tap to set monthly and recalculate ETA. Long-term compounding at 7% is available in full plan.</div>
                    </div>
                  ) : null}

                  <div style={{ display: "flex", gap: 10 }}>
                    <button type="button" className="btn primary" style={{ flex: 1, minHeight: 44 }} onClick={() => { setEditGoal(detail); setDrawerOpen(true); }}>
                      Adjust
                    </button>
                    <button type="button" className="btn" style={{ flex: 1, minHeight: 44 }} onClick={() => setDetail(null)}>
                      Close
                    </button>
                  </div>
                </>
              );
            })()}
          </div>
        ) : null}
      </BottomSheet>

      <GoalDrawer open={drawerOpen} onClose={() => { setDrawerOpen(false); setEditGoal(null); }} goal={editGoal} />
    </div>
  );
}
