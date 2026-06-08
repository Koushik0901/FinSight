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
  const [step, setStep] = useState(0);
  // assignments: categoryId → cents
  const [assignments, setAssignments] = useState<Record<string, number>>({});

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
            <div className="eyebrow" style={{ marginBottom: 16 }}>
              Estimated monthly income
            </div>
            <div
              style={{
                fontSize: 32,
                fontFamily: "var(--mono)",
                marginBottom: 8,
              }}
            >
              <span className="money">{fmt(data.incomeCents)}</span>
            </div>
            <p className="muted" style={{ fontSize: 13 }}>
              Based on your average income over the last 3 months.
            </p>
          </div>
        );
      case 1: // Essentials
        return (
          <>
            <div className="eyebrow" style={{ marginBottom: 16 }}>
              Essential expenses
            </div>
            {renderCategoryStep(true)}
          </>
        );
      case 2: // Wants
        return (
          <>
            <div className="eyebrow" style={{ marginBottom: 16 }}>
              Discretionary spending
            </div>
            {renderCategoryStep(false)}
          </>
        );
      case 3: // Goals
        return (
          <div>
            <div className="eyebrow" style={{ marginBottom: 16 }}>
              Active goals
            </div>
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
            <div className="eyebrow" style={{ marginBottom: 16 }}>
              Estimated recurring charges
            </div>
            <div
              style={{
                fontSize: 28,
                fontFamily: "var(--mono)",
                marginBottom: 8,
              }}
            >
              <span className="money">{fmt(data.recurringExpenseCents)}</span>
            </div>
            <p className="muted" style={{ fontSize: 13 }}>
              Monthly-cadence subscriptions and bills.
            </p>
          </div>
        );
      case 5: // Review
        return (
          <div>
            <div className="eyebrow" style={{ marginBottom: 16 }}>
              Review &amp; apply
            </div>
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

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 70,
        background: "var(--bg)",
        display: "flex",
        flexDirection: "column",
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 16,
          padding: "16px 24px",
          borderBottom: "1px solid var(--line)",
        }}
      >
        <button
          className="btn ghost"
          onClick={onClose}
          style={{ marginRight: "auto" }}
        >
          ✕ Close
        </button>
        <div style={{ display: "flex", gap: 8 }}>
          {STEPS.map((s, i) => (
            <button
              key={s}
              className={`btn ghost sm${i === step ? " active" : ""}`}
              style={{ opacity: i > step ? 0.4 : 1, fontWeight: i === step ? 600 : undefined }}
              onClick={() => i <= step && setStep(i)}
            >
              {i + 1}. {s}
            </button>
          ))}
        </div>
      </div>

      {/* Body */}
      <div
        style={{
          flex: 1,
          overflowY: "auto",
          padding: "32px 40px",
          maxWidth: 640,
          margin: "0 auto",
          width: "100%",
        }}
      >
        <h2 style={{ marginBottom: 24, fontWeight: 600 }}>
          Plan Next Month — {STEPS[step]}
        </h2>
        {renderStep()}
      </div>

      {/* Footer */}
      <div
        style={{
          padding: "16px 24px",
          borderTop: "1px solid var(--line)",
          display: "flex",
          justifyContent: "flex-end",
          gap: 12,
        }}
      >
        {step > 0 && (
          <button className="btn ghost" onClick={() => setStep(s => s - 1)}>
            ← Back
          </button>
        )}
        {step < STEPS.length - 1 ? (
          <button className="btn" onClick={() => setStep(s => s + 1)}>
            Next →
          </button>
        ) : (
          <button
            className="btn"
            onClick={handleApply}
            disabled={apply.isPending}
          >
            {apply.isPending ? "Applying…" : "Apply budget"}
          </button>
        )}
      </div>
    </div>
  );
}
