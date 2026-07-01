import { useState } from "react";
import { usePlanNextMonthData, useApplyNextMonthPlan } from "../api/hooks/budget";
import { type PlanAssignment } from "../api/client";
import { toast } from "sonner";

interface Props {
  onClose: () => void;
}

export default function PlanNextMonthModal({ onClose }: Props) {
  const { data, isLoading } = usePlanNextMonthData();
  const apply = useApplyNextMonthPlan();
  const [step, setStepRaw] = useState(0);
  const [reachedSteps, setReachedSteps] = useState<Set<number>>(new Set([0]));
  // assignments: categoryId → cents
  const [assignments, setAssignments] = useState<Record<string, number>>({});

  const setStep = (updater: number | ((s: number) => number)) => {
    setStepRaw(prev => {
      const next = typeof updater === "function" ? updater(prev) : updater;
      setReachedSteps(r => (r.has(next) ? r : new Set(r).add(next)));
      return next;
    });
  };

  const fmt = (cents: number) =>
    new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "USD",
      maximumFractionDigits: 0,
    }).format(cents / 100);

  const setAmt = (categoryId: string, cents: number) =>
    setAssignments(prev => ({ ...prev, [categoryId]: cents }));

  if (isLoading || !data) {
    return (
      <div
        style={{
          position: "fixed",
          inset: 0,
          zIndex: 70,
          background: "var(--bg)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <span className="muted">Loading…</span>
      </div>
    );
  }

  const STEPS = ["Income", "Essentials", "Wants", "Goals", "Recurring", "Review"];

  const handleApply = async () => {
    const list: PlanAssignment[] = Object.entries(assignments)
      .filter(([, cents]) => cents > 0)
      .map(([categoryId, amountCents]) => ({ categoryId, amountCents }));
    try {
      await apply.mutateAsync(list);
      toast.success("Next month's budget applied!");
      onClose();
    } catch (e: unknown) {
      toast.error(e instanceof Error ? e.message : "Failed to apply budget");
    }
  };

  const renderCategoryStep = (isEssentials: boolean) => {
    const cats = isEssentials
      ? data.categories.filter(cat => cat.groupLabel.toLowerCase().includes("fixed"))
      : data.categories.filter(cat => !cat.groupLabel.toLowerCase().includes("fixed"));
    return (
      <div>
        {cats.map(cat => {
          const current = assignments[cat.categoryId] ?? cat.budgetCents ?? 0;
          // Average of last 3 months of spending
          const avg = Math.round((cat.m0Cents + cat.m1Cents + cat.m2Cents) / 3);
          return (
            <div
              key={cat.categoryId}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 12,
                marginBottom: 12,
              }}
            >
              <span style={{ flex: 1 }}>{cat.label}</span>
              <span
                className="muted"
                style={{ fontSize: 12, fontFamily: "var(--mono)" }}
              >
                avg <span className="money">{fmt(avg)}</span>
              </span>
              <input
                type="number"
                value={Math.round(current / 100)}
                min={0}
                step={10}
                style={{
                  width: 80,
                  textAlign: "right",
                  padding: "4px 8px",
                  background: "var(--surface-2)",
                  border: "1px solid var(--line)",
                  borderRadius: 4,
                  color: "var(--ink)",
                  fontFamily: "var(--mono)",
                  fontSize: 13,
                }}
                onChange={e =>
                  setAmt(
                    cat.categoryId,
                    Math.round(parseFloat(e.target.value || "0") * 100),
                  )
                }
              />
            </div>
          );
        })}
      </div>
    );
  };

  const renderStep = () => {
    switch (step) {
      case 0: // Income
        return (
          <div>
            <div className="num-step">Step 1 of 6 · Income</div>
            <h1>Your estimated income.</h1>
            <p className="lead">
              Based on your average income over the last 3 months.
            </p>
            <div
              style={{
                fontSize: 32,
                fontFamily: "var(--mono)",
                marginBottom: 8,
              }}
            >
              <span className="money">{fmt(data.incomeCents)}</span>
            </div>
          </div>
        );
      case 1: // Essentials
        return (
          <div>
            <div className="num-step">Step 2 of 6 · Essentials</div>
            <h1>Essential expenses.</h1>
            <p className="lead">Fixed costs that show up every month.</p>
            {renderCategoryStep(true)}
          </div>
        );
      case 2: // Wants
        return (
          <div>
            <div className="num-step">Step 3 of 6 · Wants</div>
            <h1>Discretionary spending.</h1>
            <p className="lead">Everything outside fixed costs.</p>
            {renderCategoryStep(false)}
          </div>
        );
      case 3: // Goals
        return (
          <div>
            <div className="num-step">Step 4 of 6 · Goals</div>
            <h1>Active goals.</h1>
            <p className="lead">What you're working toward right now.</p>
            {data.goals.length === 0 ? (
              <p className="muted">No active goals.</p>
            ) : (
              data.goals.map(g => (
                <div
                  key={g.id}
                  className="card"
                  style={{ padding: "12px 16px", marginBottom: 8 }}
                >
                  <div style={{ fontWeight: 500 }}>{g.name}</div>
                  <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>
                    <span className="money">{fmt(g.currentCents)}</span> / <span className="money">{fmt(g.targetCents)}</span>
                  </div>
                </div>
              ))
            )}
          </div>
        );
      case 4: // Recurring
        return (
          <div>
            <div className="num-step">Step 5 of 6 · Recurring</div>
            <h1>Estimated recurring charges.</h1>
            <p className="lead">
              Monthly-cadence subscriptions and bills.
            </p>
            <div
              style={{
                fontSize: 28,
                fontFamily: "var(--mono)",
                marginBottom: 8,
              }}
            >
              <span className="money">{fmt(data.recurringExpenseCents)}</span>
            </div>
          </div>
        );
      case 5: // Review
        return (
          <div>
            <div className="num-step">Step 6 of 6 · Review</div>
            <h1>Review &amp; apply.</h1>
            <p className="lead">Confirm the amounts below before applying next month's budget.</p>
            {Object.entries(assignments).filter(([, v]) => v > 0).length === 0 ? (
              <p className="muted">No budget amounts set yet.</p>
            ) : (
              <table className="tbl" style={{ width: "100%" }}>
                <tbody>
                  {data.categories.map(cat => {
                    const amt = assignments[cat.categoryId] ?? 0;
                    if (amt === 0) return null;
                    return (
                      <tr key={cat.categoryId}>
                        <td>{cat.label}</td>
                        <td
                          className="num money"
                          style={{
                            textAlign: "right",
                            fontFamily: "var(--mono)",
                          }}
                        >
                          {fmt(amt)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>
        );
      default:
        return null;
    }
  };

  const renderPreview = () => {
    // Live preview: sums the currently-entered assignments (the same values
    // handleApply will send) grouped by category group, against income.
    const groupTotals = new Map<string, { color: string; cents: number }>();
    for (const cat of data.categories) {
      const cents = assignments[cat.categoryId] ?? 0;
      if (cents <= 0) continue;
      const existing = groupTotals.get(cat.groupLabel);
      if (existing) {
        existing.cents += cents;
      } else {
        groupTotals.set(cat.groupLabel, { color: cat.color, cents });
      }
    }
    const assignedTotal = [...groupTotals.values()].reduce((s, g) => s + g.cents, 0);
    const remainingCents = data.incomeCents - assignedTotal;

    return (
      <>
        <div className="eyebrow" style={{ marginBottom: 14 }}>
          <span className="dot" />Live preview
        </div>
        <div className="card" style={{ padding: 22 }}>
          <div
            className="muted"
            style={{
              fontSize: 12,
              fontFamily: "var(--mono)",
              textTransform: "uppercase",
              letterSpacing: "0.06em",
              marginBottom: 8,
            }}
          >
            Income
          </div>
          <div style={{ fontSize: 32, fontFamily: "var(--mono)", marginBottom: 16 }}>
            <span className="money">{fmt(data.incomeCents)}</span>
          </div>

          <div
            style={{
              height: 24,
              borderRadius: 6,
              background: "var(--surface-2)",
              overflow: "hidden",
              display: "flex",
              gap: 2,
            }}
          >
            {[...groupTotals.entries()].map(([label, g]) => (
              <span
                key={label}
                title={`${label} ${fmt(g.cents)}`}
                style={{
                  width: `${data.incomeCents > 0 ? Math.min(100, (g.cents / data.incomeCents) * 100) : 0}%`,
                  background: g.color,
                }}
              />
            ))}
            {remainingCents > 0 && (
              <span
                title={`Unassigned ${fmt(remainingCents)}`}
                style={{
                  flex: 1,
                  background: "var(--surface)",
                  borderLeft: "1px dashed var(--ink-faint)",
                }}
              />
            )}
          </div>

          <div style={{ marginTop: 18, display: "flex", flexDirection: "column", gap: 8 }}>
            {[...groupTotals.entries()].map(([label, g]) => (
              <div
                key={label}
                style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span className="cswatch" style={{ background: g.color }} />
                  <span style={{ fontSize: 14 }}>{label}</span>
                </div>
                <span className="num money" style={{ fontSize: 14 }}>
                  {fmt(g.cents)}
                </span>
              </div>
            ))}
            <div style={{ height: 1, background: "var(--hairline)", margin: "4px 0" }} />
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <span style={{ fontSize: 14, fontWeight: 500 }}>
                {remainingCents >= 0 ? "Unassigned" : "Over"}
              </span>
              <span
                className="num money"
                style={{
                  fontSize: 14,
                  fontWeight: 600,
                  color: remainingCents < 0 ? "var(--negative)" : undefined,
                }}
              >
                {fmt(Math.abs(remainingCents))}
              </span>
            </div>
          </div>
        </div>
      </>
    );
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 70,
        background: "var(--bg)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
      }}
    >
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
                  <button className="btn ghost" onClick={() => setStep(s => s - 1)}>
                    ← Back
                  </button>
                )}
                {step < STEPS.length - 1 ? (
                  <button className="btn primary" onClick={() => setStep(s => s + 1)}>
                    Next →
                  </button>
                ) : (
                  <button
                    className="btn primary"
                    onClick={handleApply}
                    disabled={apply.isPending}
                  >
                    {apply.isPending ? "Applying…" : "Apply budget"}
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
