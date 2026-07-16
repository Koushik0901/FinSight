import { useMemo, useState } from "react";
import {
  usePlanNextMonthData,
  useApplyNextMonthPlan,
  useUpdateGoalMonthly,
} from "../api/hooks/budget";
import { type CategoryPlanRow, type PlanAssignment } from "../api/client";
import { toast } from "sonner";
import { money } from "../utils/format";

interface Props {
  onClose: () => void;
}

const STEPS = ["Look back", "Fixed costs", "Sinking funds", "Buffer", "Goals", "Adjust", "Review"];

interface AdjustSuggestion {
  categoryId: string;
  label: string;
  suggestedCents: number;
  monthsOver: number;
}

/** Non-fixed categories over budget in >= 2 of the last 3 months, sorted worst first, capped at 3. */
function computeAdjustSuggestions(categories: CategoryPlanRow[]): AdjustSuggestion[] {
  const suggestions: AdjustSuggestion[] = [];
  for (const cat of categories) {
    if (cat.groupLabel.toLowerCase().includes("fixed")) continue; // has its own step
    if (cat.budgetCents <= 0) continue;
    const months = [cat.m0Cents, cat.m1Cents, cat.m2Cents];
    const monthsOver = months.filter((m) => m > cat.budgetCents).length;
    if (monthsOver >= 2) {
      const maxSpend = Math.max(...months);
      const suggestedCents = Math.ceil(maxSpend / 1000) * 1000; // round up to the nearest $10
      suggestions.push({ categoryId: cat.categoryId, label: cat.label, suggestedCents, monthsOver });
    }
  }
  return suggestions.sort((a, b) => b.monthsOver - a.monthsOver).slice(0, 3);
}

export default function PlanNextMonthModal({ onClose }: Props) {
  const { data, isLoading } = usePlanNextMonthData();
  const apply = useApplyNextMonthPlan();
  const updateGoalMonthly = useUpdateGoalMonthly();
  const [step, setStepRaw] = useState(0);
  const [reachedSteps, setReachedSteps] = useState<Set<number>>(new Set([0]));
  // Category budget assignments: categoryId → cents.
  const [assignments, setAssignments] = useState<Record<string, number>>({});
  // Monthly-contribution overrides for sinking funds / goals: goalId → cents.
  const [sinkingAssignments, setSinkingAssignments] = useState<Record<string, number>>({});
  const [goalAssignments, setGoalAssignments] = useState<Record<string, number>>({});
  const [buffer, setBuffer] = useState(0);
  const [acceptedAdjustments, setAcceptedAdjustments] = useState<Set<string>>(new Set());

  const setStep = (updater: number | ((s: number) => number)) => {
    setStepRaw((prev) => {
      const next = typeof updater === "function" ? updater(prev) : updater;
      setReachedSteps((r) => (r.has(next) ? r : new Set(r).add(next)));
      return next;
    });
  };

  const fmt = (cents: number) => money(cents);
  const setAmt = (categoryId: string, cents: number) => setAssignments((prev) => ({ ...prev, [categoryId]: cents }));
  const setSinkingAmt = (goalId: string, cents: number) => setSinkingAssignments((prev) => ({ ...prev, [goalId]: cents }));
  const setGoalAmt = (goalId: string, cents: number) => setGoalAssignments((prev) => ({ ...prev, [goalId]: cents }));

  const suggestions = useMemo(() => (data ? computeAdjustSuggestions(data.categories) : []), [data]);

  if (isLoading || !data) {
    return (
      <div style={{ position: "fixed", inset: 0, zIndex: 70, background: "var(--bg)", display: "flex", alignItems: "center", justifyContent: "center" }}>
        <span className="muted">Loading…</span>
      </div>
    );
  }

  const acceptAdjustment = (s: AdjustSuggestion) => {
    setAcceptedAdjustments((prev) => new Set(prev).add(s.categoryId));
    setAmt(s.categoryId, s.suggestedCents);
  };

  const fixedTotal = data.categories
    .filter((c) => c.groupLabel.toLowerCase().includes("fixed"))
    .reduce((sum, c) => sum + (assignments[c.categoryId] ?? c.budgetCents ?? 0), 0);
  const sinkingTotal = Object.values(sinkingAssignments).reduce((sum, v) => sum + v, 0);
  const goalTotal = Object.values(goalAssignments).reduce((sum, v) => sum + v, 0);
  const planned = fixedTotal + sinkingTotal + buffer + goalTotal;
  const remainingCents = data.incomeCents - planned;

  const handleApply = async () => {
    const categoryAssignments: PlanAssignment[] = Object.entries(assignments)
      .filter(([, cents]) => cents > 0)
      .map(([categoryId, amountCents]) => ({ categoryId, amountCents }));
    try {
      await apply.mutateAsync(categoryAssignments);
      const monthlyUpdates = [...Object.entries(sinkingAssignments), ...Object.entries(goalAssignments)];
      for (const [id, monthlyCents] of monthlyUpdates) {
        await updateGoalMonthly.mutateAsync({ id, monthlyCents });
      }
      toast.success("Next month's budget applied!");
      onClose();
    } catch (e: unknown) {
      toast.error(e instanceof Error ? e.message : "Failed to apply budget");
    }
  };

  const renderFixedCostsStep = () => (
    <div>
      {data.categories
        .filter((cat) => cat.groupLabel.toLowerCase().includes("fixed"))
        .map((cat) => {
          const current = assignments[cat.categoryId] ?? cat.budgetCents ?? 0;
          const avg = Math.round((cat.m0Cents + cat.m1Cents + cat.m2Cents) / 3);
          return (
            <div key={cat.categoryId} style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 12 }}>
              <span style={{ flex: 1 }}>{cat.label}</span>
              <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>
                avg <span className="money">{fmt(avg)}</span>
              </span>
              <input
                type="number"
                value={Math.round(current / 100)}
                min={0}
                step={10}
                style={{ width: 80, textAlign: "right", padding: "4px 8px", background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 4, color: "var(--ink)", fontFamily: "var(--mono)", fontSize: 13 }}
                onChange={(e) => setAmt(cat.categoryId, Math.round(parseFloat(e.target.value || "0") * 100))}
              />
            </div>
          );
        })}
    </div>
  );

  const renderStep = () => {
    switch (step) {
      case 0: // Look back
        return (
          <div>
            <div className="num-step">Step 1 of 7 · Look back</div>
            <h1>First, look back.</h1>
            <p className="lead">Before deciding what next month should be, a quick view of how last month actually played out — no shame, no celebration, just the facts.</p>
            {data.lookBack.length === 0 ? (
              <p className="muted">Not enough budgeted history yet to draw any facts from last month.</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 10, maxWidth: 460 }}>
                {data.lookBack.map((f) => (
                  <div key={`${f.categoryId}-${f.kind}`} className="card tight" style={{ padding: 14 }}>
                    <div className="strong" style={{ fontSize: 14 }}>
                      {f.kind === "over" && <>{f.categoryLabel} ran <span className="money">{fmt(f.amountCents)}</span> over budget.</>}
                      {f.kind === "under" && <>{f.categoryLabel} came in <span className="money">{fmt(f.amountCents)}</span> under budget.</>}
                      {f.kind === "streak" && <>{f.categoryLabel} sat at $0 — {f.streakMonths} months in a row.</>}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        );
      case 1: // Fixed costs
        return (
          <div>
            <div className="num-step">Step 2 of 7 · Fixed costs</div>
            <h1>What's already spoken for?</h1>
            <p className="lead">Things that show up whether you plan for them or not.</p>
            {renderFixedCostsStep()}
          </div>
        );
      case 2: // Sinking funds
        return (
          <div>
            <div className="num-step">Step 3 of 7 · Sinking funds</div>
            <h1>What's coming that isn't monthly?</h1>
            <p className="lead">Insurance renewals, annual bills, the irregular expenses that ambush you if you don't set them aside a little at a time.</p>
            {data.sinkingFunds.length === 0 ? (
              <p className="muted">No sinking funds yet — create one on the Goals screen with type "Sinking fund".</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {data.sinkingFunds.map((s) => {
                  const val = sinkingAssignments[s.id] ?? s.monthlyCents;
                  return (
                    <div key={s.id} style={{ padding: 14, background: "var(--surface-2)", borderRadius: 8 }}>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                        <div>
                          <div style={{ fontSize: 14, fontWeight: 500 }}>{s.name}</div>
                          <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>
                            <span className="money">{fmt(s.currentCents)}</span> of <span className="money">{fmt(s.targetCents)}</span>
                          </div>
                        </div>
                        <div className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>
                          {fmt(val)}<span style={{ fontSize: 13, color: "var(--ink-mute)", marginLeft: 4 }}>/mo</span>
                        </div>
                      </div>
                      <input
                        type="range"
                        min="0"
                        max="50000"
                        step="1000"
                        value={val}
                        onChange={(e) => setSinkingAmt(s.id, parseInt(e.target.value, 10))}
                        style={{ width: "100%", marginTop: 10, accentColor: "var(--accent)" }}
                      />
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      case 3: // Buffer
        return (
          <div>
            <div className="num-step">Step 4 of 7 · Buffer</div>
            <h1>How much slack should next month have?</h1>
            <p className="lead">Money set aside but not assigned to anything yet — deliberate breathing room, not a leftover.</p>
            <div style={{ maxWidth: 460 }}>
              <div style={{ padding: 18, background: "var(--surface-2)", borderRadius: 10, marginTop: 8 }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                  <span style={{ fontSize: 14 }}>Buffer</span>
                  <span className="figure" style={{ fontSize: 26, color: "var(--accent)" }}>{fmt(buffer)}</span>
                </div>
                <input
                  type="range"
                  min="0"
                  max="200000"
                  step="5000"
                  value={buffer}
                  onChange={(e) => setBuffer(parseInt(e.target.value, 10))}
                  style={{ width: "100%", marginTop: 12, accentColor: "var(--accent)" }}
                />
              </div>
            </div>
          </div>
        );
      case 4: // Goals
        return (
          <div>
            <div className="num-step">Step 5 of 7 · Goals</div>
            <h1>What are we moving toward?</h1>
            <p className="lead">Tune what you'll contribute to each goal this month.</p>
            {data.goals.length === 0 ? (
              <p className="muted">No active goals.</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {data.goals.map((g) => {
                  const val = goalAssignments[g.id] ?? g.monthlyCents;
                  return (
                    <div key={g.id} style={{ padding: 14, background: "var(--surface-2)", borderRadius: 8 }}>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                        <div>
                          <div style={{ fontSize: 14, fontWeight: 500 }}>{g.name}</div>
                          <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>
                            <span className="money">{fmt(g.currentCents)}</span> of <span className="money">{fmt(g.targetCents)}</span>
                          </div>
                        </div>
                        <div className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>
                          {fmt(val)}<span style={{ fontSize: 13, color: "var(--ink-mute)", marginLeft: 4 }}>/mo</span>
                        </div>
                      </div>
                      <input
                        type="range"
                        min="0"
                        max="200000"
                        step="5000"
                        value={val}
                        onChange={(e) => setGoalAmt(g.id, parseInt(e.target.value, 10))}
                        style={{ width: "100%", marginTop: 10, accentColor: "var(--accent)" }}
                      />
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      case 5: // Adjust
        return (
          <div>
            <div className="num-step">Step 6 of 7 · Adjust</div>
            <h1>What needs to shift?</h1>
            <p className="lead">Categories that ran over budget in at least 2 of the last 3 months — based on your own history, not a guess.</p>
            {suggestions.length === 0 ? (
              <p className="muted">Nothing stands out — your non-fixed categories have mostly stayed within budget.</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {suggestions.map((s) => {
                  const on = acceptedAdjustments.has(s.categoryId);
                  return (
                    <div
                      key={s.categoryId}
                      onClick={() => acceptAdjustment(s)}
                      style={{ padding: 14, background: on ? "var(--accent-2)" : "var(--surface-2)", border: `1px solid ${on ? "var(--accent-3)" : "var(--line)"}`, borderRadius: 8, cursor: "pointer" }}
                    >
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                        <span style={{ fontSize: 14, fontWeight: 500 }}>Raise {s.label} to {fmt(s.suggestedCents)}</span>
                        <span className={`tog ${on ? "on" : ""}`} />
                      </div>
                      <div className="muted" style={{ fontSize: 13, marginTop: 6 }}>Over budget {s.monthsOver} of the last 3 months.</div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      case 6: // Review
        return (
          <div>
            <div className="num-step">Step 7 of 7 · Review</div>
            <h1>Review &amp; apply.</h1>
            <p className="lead">Confirm the amounts below before applying next month's plan.</p>
            <table className="tbl" style={{ width: "100%" }}>
              <tbody>
                <tr><td>Fixed costs</td><td className="right" style={{ fontFamily: "var(--mono)" }}><span className="money">{fmt(fixedTotal)}</span></td></tr>
                <tr><td>Sinking funds</td><td className="right" style={{ fontFamily: "var(--mono)" }}><span className="money">{fmt(sinkingTotal)}</span></td></tr>
                <tr><td>Buffer</td><td className="right" style={{ fontFamily: "var(--mono)" }}><span className="money">{fmt(buffer)}</span></td></tr>
                <tr><td>Goals</td><td className="right" style={{ fontFamily: "var(--mono)" }}><span className="money">{fmt(goalTotal)}</span></td></tr>
              </tbody>
            </table>
          </div>
        );
      default:
        return null;
    }
  };

  const renderPreview = () => {
    const segments = [
      { key: "fixed", label: "Fixed costs", cents: fixedTotal },
      { key: "sinks", label: "Sinking funds", cents: sinkingTotal },
      { key: "buffer", label: "Buffer", cents: buffer },
      { key: "goals", label: "Goals", cents: goalTotal },
    ].filter((s) => s.cents > 0);

    return (
      <>
        <div className="eyebrow" style={{ marginBottom: 14 }}>
          <span className="dot" />Live preview
        </div>
        <div className="card" style={{ padding: 22 }}>
          <div className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)", textTransform: "uppercase", letterSpacing: "0.06em", marginBottom: 8 }}>
            Income
          </div>
          <div style={{ fontSize: 32, fontFamily: "var(--mono)", marginBottom: 16 }}>
            <span className="money">{fmt(data.incomeCents)}</span>
          </div>

          <div style={{ height: 24, borderRadius: 6, background: "var(--surface-2)", overflow: "hidden", display: "flex", gap: 2 }}>
            {segments.map((s) => (
              <span
                key={s.key}
                title={`${s.label} ${fmt(s.cents)}`}
                style={{ width: `${data.incomeCents > 0 ? Math.min(100, (s.cents / data.incomeCents) * 100) : 0}%`, background: "var(--accent)" }}
              />
            ))}
            {remainingCents > 0 && (
              <span title={`Unassigned ${fmt(remainingCents)}`} style={{ flex: 1, background: "var(--surface)", borderLeft: "1px dashed var(--ink-faint)" }} />
            )}
          </div>

          <div style={{ marginTop: 18, display: "flex", flexDirection: "column", gap: 8 }}>
            {segments.map((s) => (
              <div key={s.key} style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <span style={{ fontSize: 14 }}>{s.label}</span>
                <span className="num money" style={{ fontSize: 14 }}>{fmt(s.cents)}</span>
              </div>
            ))}
            <div style={{ height: 1, background: "var(--hairline)", margin: "4px 0" }} />
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <span style={{ fontSize: 14, fontWeight: 500 }}>{remainingCents >= 0 ? "Unassigned" : "Over"}</span>
              <span className="num money" style={{ fontSize: 14, fontWeight: 600, color: remainingCents < 0 ? "var(--negative)" : undefined }}>
                {fmt(Math.abs(remainingCents))}
              </span>
            </div>
          </div>
        </div>
      </>
    );
  };

  return (
    <div style={{ position: "fixed", inset: 0, zIndex: 70, background: "var(--bg)", display: "flex", alignItems: "center", justifyContent: "center", padding: 24 }}>
      <div className="onb-shell" style={{ width: "100%", maxWidth: 1120 }}>
        <header className="onb-top">
          <div className="brand" style={{ padding: 0 }}>
            <div className="mark" aria-hidden="true" />
            <div className="wm">FinSight</div>
          </div>
          <nav className="onb-steps" aria-label="Plan next month progress">
            {STEPS.map((s, i) => {
              const reached = reachedSteps.has(i);
              return (
                <button
                  key={s}
                  className={`onb-step-pip ${i === step ? "cur" : ""} ${reached ? "done" : ""}`}
                  disabled={!reached}
                  onClick={() => reached && setStep(i)}
                  aria-current={i === step ? "step" : undefined}
                  aria-label={`Go to ${s} step`}
                  title={s}
                  type="button"
                />
              );
            })}
          </nav>
          <button className="btn ghost sm" onClick={onClose}>
            ✕ Close
          </button>
        </header>

        <section className="onb-stage" aria-label="Plan next month steps">
          <div className="onb-split">
            <div className="onb-left">
              {renderStep()}

              <div className="onb-actions" style={{ marginTop: 24 }}>
                {step > 0 && (
                  <button className="btn ghost" onClick={() => setStep((s) => s - 1)}>
                    ← Back
                  </button>
                )}
                {step < STEPS.length - 1 ? (
                  <button className="btn primary" onClick={() => setStep((s) => s + 1)}>
                    Next →
                  </button>
                ) : (
                  <button
                    className="btn primary"
                    onClick={() => void handleApply()}
                    disabled={apply.isPending || updateGoalMonthly.isPending}
                  >
                    {apply.isPending || updateGoalMonthly.isPending ? "Applying…" : "Apply budget"}
                  </button>
                )}
              </div>
            </div>

            <div className="onb-right">{renderPreview()}</div>
          </div>
        </section>
      </div>
    </div>
  );
}
