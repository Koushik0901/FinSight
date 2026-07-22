import { useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { useMonthClose, useMonthCloses, useSaveMonthClose } from "../api/hooks/reports";
import { money } from "../utils/format";

/** The month the close targets: from ?year=&month=, else the month that just ended. */
function useTargetMonth() {
  const [params] = useSearchParams();
  return useMemo(() => {
    const prev = new Date(new Date().getFullYear(), new Date().getMonth() - 1, 1);
    const year = Number(params.get("year")) || prev.getFullYear();
    const month = Number(params.get("month")) || prev.getMonth() + 1;
    return { year, month };
  }, [params]);
}

const STATUS_LABEL: Record<string, string> = {
  not_started: "Not started",
  in_progress: "In progress",
  completed: "Completed",
  skipped: "Skipped",
};

export default function MonthClose() {
  const navigate = useNavigate();
  const { year, month } = useTargetMonth();
  const { data: view, isLoading, error } = useMonthClose(year, month);
  const { data: pastCloses = [] } = useMonthCloses();
  const save = useSaveMonthClose();

  const [notes, setNotes] = useState<string | null>(null);
  const [acked, setAcked] = useState<Set<string>>(new Set());

  // Seed local notes from the loaded view once.
  const effectiveNotes = notes ?? view?.notes ?? "";
  const completed = view?.status === "completed";

  const persist = (status: "in_progress" | "completed" | "skipped") => {
    save.mutate(
      {
        year,
        month,
        status,
        notes: effectiveNotes || null,
        acknowledgedFlagIds: [...acked],
      },
      {
        onSuccess: () => {
          const verb = status === "completed" ? "closed" : status === "skipped" ? "skipped" : "saved";
          toast.success(`${view?.monthLabel ?? "Month"} ${verb}`);
        },
        onError: () => toast.error("Could not update the close"),
      },
    );
  };

  if (isLoading) return <div className="stub">Loading month close…</div>;
  if (error || !view) return <div className="stub" role="alert">Could not load the month close.</div>;

  const s = view.snapshot;
  // Acknowledged state comes from the frozen record when completed, else local.
  const isAcked = (id: string, frozen: boolean) => (completed ? frozen : acked.has(id));

  return (
    <div className="screen screen-month-close">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Month-end close</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Close out {view.monthLabel}.</h1>
        </div>
        <span className="chip" style={{ alignSelf: "flex-start" }}>{STATUS_LABEL[view.status] ?? view.status}</span>
      </div>

      {/* Drift — a completed close whose numbers have since moved. */}
      {completed && view.drift.length > 0 && (
        <div className="card" style={{ borderColor: "var(--warning)", marginBottom: 20 }}>
          <div className="eyebrow" style={{ color: "var(--warning)" }}>Numbers have drifted since you closed</div>
          <div className="stack stack-xs" style={{ marginTop: 8 }}>
            {view.drift.map((d) => (
              <div key={d.label} className="row" style={{ justifyContent: "space-between", fontSize: 13 }}>
                <span className="muted">{d.label}</span>
                <span className="num">recorded <span className="money">{money(d.recordedCents)}</span> → now <span className="money">{money(d.currentCents)}</span></span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Data quality */}
      <section className="section">
        <div className="card">
          <div className="eyebrow" style={{ marginBottom: 2 }}>Verify the month&apos;s data</div>
          <div className="muted" style={{ fontSize: 13, marginBottom: 12 }}>
            Resolve what you can, acknowledge the rest — nothing here blocks closing.
          </div>
          {view.flags.length === 0 ? (
            <div className="muted" style={{ fontSize: 13 }}>Nothing flagged. The month&apos;s data looks clean.</div>
          ) : (
            view.flags.map((f) => (
              <div key={f.id} className="row" style={{ gap: 12, alignItems: "center", padding: "12px 0", borderTop: "1px solid var(--hairline)" }}>
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={isAcked(f.id, f.acknowledged)}
                  aria-label={`Acknowledge: ${f.title}`}
                  disabled={completed}
                  onClick={() => setAcked((prev) => { const n = new Set(prev); n.has(f.id) ? n.delete(f.id) : n.add(f.id); return n; })}
                  style={{ width: 18, height: 18, flexShrink: 0, borderRadius: 5, border: "1.5px solid var(--line)", background: isAcked(f.id, f.acknowledged) ? "var(--accent)" : "var(--surface-2)", cursor: completed ? "default" : "pointer" }}
                />
                <div className="grow" style={{ minWidth: 0 }}>
                  <div style={{ fontSize: 13.5, fontWeight: 500 }}>{f.title}</div>
                  <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>{f.detail}</div>
                </div>
                {typeof f.count === "number" && f.count > 0 && <span className="chip">{f.count}</span>}
                {!completed && (
                  <button type="button" className="btn outline sm" onClick={() => navigate(f.actionRoute)}>Review →</button>
                )}
              </div>
            ))
          )}
        </div>
      </section>

      {/* Money review */}
      <section className="section">
        <div className="card">
          <div className="eyebrow" style={{ marginBottom: 2 }}>The month in numbers</div>
          <div className="muted" style={{ fontSize: 13, marginBottom: 14 }}>
            The same figures every screen shows — {completed ? "frozen when you closed." : "frozen when you complete the close."}
          </div>
          <div className="stat-row">
            <div className="stat"><div className="label">Income</div><div className="value money">{money(s.incomeCents)}</div></div>
            <div className="stat"><div className="label">Spending</div><div className="value money">{money(s.expenseCents)}</div></div>
            <div className="stat"><div className="label">Saved</div><div className={`value money ${s.savingsCents >= 0 ? "pos" : ""}`}>{money(s.savingsCents)}</div></div>
            <div className="stat"><div className="label">Savings rate</div><div className="value">{s.savingsRatePct}%</div></div>
            <div className="stat"><div className="label">Net worth</div><div className="value money">{money(s.netWorthCents)}</div></div>
            <div className="stat"><div className="label">Debt</div><div className="value money">{money(s.debtTotalCents)}</div></div>
          </div>
          {(s.overBudgetCategories.length > 0 || s.subscriptionChangeCount > 0) && (
            <div className="row wrap" style={{ gap: 7, marginTop: 14 }}>
              {s.overBudgetCategories.map((c) => (
                <span key={c} className="chip" style={{ color: "var(--negative)", borderColor: "var(--negative)" }}>Over budget: {c}</span>
              ))}
              {s.subscriptionChangeCount > 0 && (
                <span className="chip" style={{ color: "var(--warning)", borderColor: "var(--warning)" }}>
                  {s.subscriptionChangeCount} subscription {s.subscriptionChangeCount === 1 ? "change" : "changes"}
                </span>
              )}
            </div>
          )}
        </div>
      </section>

      {/* Notes */}
      <section className="section">
        <div className="card">
          <div className="eyebrow" style={{ marginBottom: 2 }}>Notes for this month</div>
          <div className="muted" style={{ fontSize: 13, marginBottom: 10 }}>What happened that the numbers don&apos;t explain? Recorded with the close.</div>
          <textarea
            className="control"
            style={{ width: "100%", minHeight: 72, resize: "vertical" }}
            placeholder="e.g. One-off medical bill in Dining; bonus lands next month."
            value={effectiveNotes}
            disabled={completed}
            onChange={(e) => setNotes(e.target.value)}
          />
        </div>
      </section>

      {/* Actions */}
      <section className="section">
        <div className="card">
          <div className="row wrap" style={{ gap: 10, alignItems: "center" }}>
            {completed ? (
              <>
                <button type="button" className="btn outline" disabled={save.isPending} onClick={() => persist("in_progress")}>Reopen</button>
                <span className="muted" style={{ fontSize: 12 }}>Reopening keeps the recorded snapshot; re-completing records a new one.</span>
              </>
            ) : (
              <>
                <button type="button" className="btn primary" disabled={save.isPending} onClick={() => persist("completed")}>Complete close</button>
                <button type="button" className="btn outline" disabled={save.isPending} onClick={() => persist("in_progress")}>Save for later</button>
                <button type="button" className="btn ghost" disabled={save.isPending} onClick={() => persist("skipped")}>Skip this month</button>
              </>
            )}
            <span className="grow" />
            <button type="button" className="btn ghost" onClick={() => navigate("/budget")}>Plan next month →</button>
          </div>
          {completed && view.completedAt && (
            <div className="muted" style={{ fontSize: 12, marginTop: 10 }}>Recorded {new Date(view.completedAt).toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric" })}.</div>
          )}
        </div>
      </section>

      {/* Past closes */}
      {pastCloses.length > 0 && (
        <section className="section">
          <div className="eyebrow" style={{ marginBottom: 10 }}>Past closes</div>
          <div className="card flush">
            {pastCloses.map((c) => (
              <button
                key={`${c.year}-${c.month}`}
                type="button"
                onClick={() => navigate(`/close?year=${c.year}&month=${c.month}`)}
                style={{ width: "100%", textAlign: "left", display: "grid", gridTemplateColumns: "1fr auto auto", gap: 14, alignItems: "center", padding: "14px 16px", borderBottom: "1px solid var(--hairline)", background: "transparent" }}
              >
                <div>{c.monthLabel}</div>
                <span className="chip">{STATUS_LABEL[c.status] ?? c.status}</span>
                <span className="num money">{money(c.netWorthCents)}</span>
              </button>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
