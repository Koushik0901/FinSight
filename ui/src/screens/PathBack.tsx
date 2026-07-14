import { useState } from "react";
import { toast } from "sonner";
import { usePathBack, useSetSpendingAnnotation } from "../api/hooks/spending";
import { useDebouncedValue } from "../utils/useDebouncedValue";
import { isTauriRuntime } from "../utils/runtime";
import { money } from "../utils/format";
import type { Driver, PeriodClass } from "../api/client";
import Card from "../components/Card";
import Button from "../components/Button";
import Badge from "../components/Badge";
import Input from "../components/Input";
import EmptyState from "../components/EmptyState";
import { CopilotNudge } from "../components/CopilotNudge";
import * as I from "../components/Icons";

const CLASS_BADGE: Record<PeriodClass, { tone: "warning" | "accent" | "positive" | "default"; label: string }> = {
  regime_shift: { tone: "warning", label: "Regime shift — not a blip" },
  episodic_spike: { tone: "accent", label: "One-month spike" },
  normal: { tone: "positive", label: "Within your normal" },
  insufficient_history: { tone: "default", label: "Not enough history yet" },
};

// Human phrasing for the raw Mechanism enum. Anything not listed here (e.g.
// "stopped", "price_down", "frequency_down", "flat") falls back to the raw
// value — still readable, just less polished.
const MECHANISM_LABELS: Record<string, string> = {
  new: "new",
  frequency_up: "more often",
  price_up: "pricier",
  mixed: "more + pricier",
};

function mechanismLabel(mechanism: string): string {
  return MECHANISM_LABELS[mechanism] ?? mechanism;
}

function DriverRow({
  driver,
  actionable,
  pending,
  onAnnotate,
}: {
  driver: Driver;
  actionable: boolean;
  pending: boolean;
  onAnnotate?: (verdict: string) => void;
}) {
  return (
    <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 10, padding: "9px 0", borderBottom: "1px solid var(--line)" }}>
      <div className="row-sm wrap" style={{ minWidth: 0, alignItems: "center" }}>
        <span style={{ fontWeight: 500, fontSize: 13.5, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {driver.display}
        </span>
        <span className="chip" style={{ fontSize: 10.5 }}>{mechanismLabel(driver.mechanism)}</span>
      </div>
      <div className="row-sm" style={{ flexShrink: 0, alignItems: "center", gap: 8 }}>
        <span className="num money" style={{ fontSize: 13, color: "var(--accent)", fontWeight: 600 }}>
          +{money(driver.delta_cents)}
        </span>
        {actionable &&
          (driver.user_verdict ? (
            <>
              <span className="muted" style={{ fontSize: 12 }}>
                · {driver.user_verdict === "one_off" ? "one-time" : "kept"}
              </span>
              <Button size="sm" variant="ghost" onClick={() => onAnnotate?.("reset")} loading={pending}>
                Undo
              </Button>
            </>
          ) : (
            <>
              <Button size="sm" variant="ghost" onClick={() => onAnnotate?.("expected")} loading={pending}>
                Keep
              </Button>
              <Button size="sm" variant="ghost" onClick={() => onAnnotate?.("one_off")} loading={pending}>
                One-time
              </Button>
            </>
          ))}
      </div>
    </div>
  );
}

export default function PathBack() {
  const [period] = useState<string | null>(null);
  const [targetInput, setTargetInput] = useState("");
  const parsedTarget = targetInput.trim() === "" ? null : Number(targetInput);
  const targetDollars = parsedTarget !== null && !Number.isNaN(parsedTarget) ? parsedTarget : null;
  const debouncedTargetDollars = useDebouncedValue(targetDollars, 400);
  const targetMonthlyCents = debouncedTargetDollars !== null ? Math.round(debouncedTargetDollars * 100) : null;

  const { data: view, isLoading, error } = usePathBack(period, targetMonthlyCents);
  const annotate = useSetSpendingAnnotation();
  const [pendingKey, setPendingKey] = useState<string | null>(null);

  if (!isTauriRuntime()) {
    return <div className="stub">Open the desktop app to see your path back.</div>;
  }
  if (isLoading) {
    return <div className="stub">Charting your path back…</div>;
  }
  if (error) {
    return <div className="stub" role="alert">Couldn't load your path back.</div>;
  }
  if (!view) {
    return (
      <EmptyState
        icon={<I.Flow style={{ color: "var(--ink-mute)", width: 40, height: 40 }} />}
        title="No spending to analyze yet"
        description="Import some transactions and come back."
      />
    );
  }

  const { assessment, plan } = view;
  const classBadge = CLASS_BADGE[assessment.class];
  const gapCents = plan.recent_monthly_cents - plan.baseline_monthly_cents;
  const hasTargetVerdict = plan.target_monthly_cents != null;
  const structuralGap = plan.structural_gap_cents;
  const barMax = Math.max(plan.projected_after_levers_cents + (structuralGap ?? 0), plan.target_monthly_cents ?? 0, 1);
  const projectedPct = Math.max(0, Math.min(100, (plan.projected_after_levers_cents / barMax) * 100));
  const structuralPct = structuralGap != null && structuralGap > 0 ? Math.max(0, Math.min(100, (structuralGap / barMax) * 100)) : 0;

  const handleAnnotate = (driver: Driver, verdict: string) => {
    setPendingKey(driver.merchant_key);
    annotate.mutate(
      { merchantKey: driver.merchant_key, verdict },
      {
        onSuccess: () => toast.success(verdict === "reset" ? "Verdict cleared" : "Got it — remembered"),
        onError: () => toast.error("Could not save that"),
        onSettled: () => setPendingKey(null),
      },
    );
  };

  return (
    <div className="screen">
      <header className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Path back · {view.period}
          </div>
          <h1>Getting back to your normal.</h1>
        </div>
        <div className="row-md wrap" style={{ alignItems: "center" }}>
          <CopilotNudge
            prompt="Help me build a plan to get my spending back to my normal."
            label="Ask Copilot to plan it"
            variant="accent"
          />
          <Badge tone={classBadge.tone}>{classBadge.label}</Badge>
        </div>
      </header>

      <div className="stat-row" style={{ gridTemplateColumns: "repeat(3, 1fr)" }}>
        <div className="stat">
          <div className="label">Recent</div>
          <div className="value money">{money(plan.recent_monthly_cents)}</div>
          <div className="sub">per month</div>
        </div>
        <div className="stat">
          <div className="label">Your normal</div>
          <div className="value money">{money(plan.baseline_monthly_cents)}</div>
          <div className="sub">median · 12 mo</div>
        </div>
        <div
          className="stat"
          style={gapCents > 0 ? { background: "linear-gradient(180deg, var(--warning-2) 0%, var(--surface) 70%)", borderColor: "var(--warning)" } : undefined}
        >
          <div className="label" style={gapCents > 0 ? { color: "var(--warning)" } : undefined}>The gap</div>
          <div className="value money">{money(gapCents)}</div>
          <div className="sub">Recent vs. normal</div>
        </div>
      </div>

      <Card style={{ marginTop: 18 }}>
        <div className="row-md wrap" style={{ alignItems: "flex-end", justifyContent: "space-between" }}>
          <Input
            label="Target"
            type="number"
            inputMode="decimal"
            placeholder="e.g. 3000"
            value={targetInput}
            onChange={(e) => setTargetInput(e.target.value)}
            style={{ maxWidth: 180 }}
          />
        </div>

        {hasTargetVerdict && (
          <div className="stack stack-sm" style={{ marginTop: 16 }}>
            <div style={{ height: 10, borderRadius: 999, background: "var(--surface-2)", overflow: "hidden", display: "flex" }}>
              <span style={{ width: `${projectedPct}%`, background: "var(--accent)", height: "100%" }} />
              {structuralPct > 0 && <span style={{ width: `${structuralPct}%`, background: "var(--warning)", height: "100%" }} />}
            </div>
            <p className="muted" style={{ fontSize: 13, margin: 0 }}>{plan.note}</p>
            {structuralGap != null && structuralGap > 0 && (
              <p className="muted" style={{ fontSize: 13, margin: 0 }}>
                <span className="money">{money(structuralGap)}</span> of that is structural — a floor these cuts don't reach.
              </p>
            )}
          </div>
        )}
      </Card>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit,minmax(260px,1fr))", gap: 12, marginTop: 18 }}>
        <Card
          header={
            <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
              <div className="h3">Your levers · trim these</div>
              <span className="num money" style={{ fontSize: 14, color: "var(--accent)", fontWeight: 600 }}>
                ~{money(plan.recoverable_recurring_cents)}
              </span>
            </div>
          }
        >
          {plan.levers.length === 0 ? (
            <p className="muted" style={{ margin: 0 }}>No recurring levers — nice.</p>
          ) : (
            <div className="stack stack-xs">
              {plan.levers.map((driver) => (
                <DriverRow
                  key={driver.merchant_key}
                  driver={driver}
                  actionable
                  pending={annotate.isPending && pendingKey === driver.merchant_key}
                  onAnnotate={(verdict) => handleAnnotate(driver, verdict)}
                />
              ))}
            </div>
          )}
        </Card>

        <Card
          tone="muted"
          header={
            <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
              <div className="h3">Self-correcting · leave them</div>
              <span className="num money" style={{ fontSize: 14, color: "var(--ink-mute)", fontWeight: 600 }}>
                ~{money(plan.self_correcting_cents)}
              </span>
            </div>
          }
          footer={
            <p className="muted" style={{ margin: 0, fontSize: 12.5 }}>
              Already behind you — no action. If any recurs, it moves to your levers automatically.
            </p>
          }
        >
          {plan.self_correcting.length === 0 ? (
            <p className="muted" style={{ margin: 0 }}>Nothing lapsing on its own right now.</p>
          ) : (
            <div className="stack stack-xs">
              {plan.self_correcting.map((driver) => (
                <DriverRow key={driver.merchant_key} driver={driver} actionable={false} pending={false} />
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}
