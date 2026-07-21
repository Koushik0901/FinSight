import Drawer from "./Drawer";
import { useMetricExplanations } from "../api/hooks/metrics";
import type { MetricExplanation, MetricValue } from "../api/client";
import { money } from "../utils/format";

/**
 * "Explain this number" — a reusable inspector that shows how one headline
 * metric is produced: its definition, the inputs that fed it, what's excluded,
 * the assumptions and period, and any data-quality warnings. The figures come
 * from the same shared metrics layer the dashboard reads, so an explanation can
 * never disagree with the number shown; a deliberately withheld figure shows
 * the reason in place of a fabricated number.
 */
export default function ExplainInspector({
  metricKey,
  memberId,
  currency,
  onClose,
}: {
  /** Stable metric key to explain (e.g. "net_worth"), or null when closed. */
  metricKey: string | null;
  memberId?: string | null;
  currency?: string;
  onClose: () => void;
}) {
  const { data: explanations, isLoading } = useMetricExplanations(memberId);
  const explanation = metricKey ? explanations?.[metricKey] : undefined;
  const open = metricKey !== null;

  return (
    <Drawer open={open} onClose={onClose} title={explanation?.label ?? "How this is calculated"} width={440}>
      {isLoading && !explanation ? (
        <div className="muted" style={{ padding: "8px 0" }}>Loading…</div>
      ) : explanation ? (
        <ExplanationBody explanation={explanation} currency={currency} />
      ) : (
        <div className="muted" style={{ padding: "8px 0" }}>
          No explanation is available for this metric yet.
        </div>
      )}
    </Drawer>
  );
}

/** Format a metric value for display. A `withheld` value has no figure — the
 *  caller renders the "Not shown yet" state and relies on the warnings. */
function valueText(value: MetricValue, currency?: string): string | null {
  switch (value.kind) {
    case "money":
      return money(value.cents, currency ? { currency } : undefined);
    case "percent":
      return `${value.pct}%`;
    case "months":
      return `${value.months.toFixed(1)} months`;
    case "days":
      return `${value.days} ${value.days === 1 ? "day" : "days"}`;
    case "withheld":
      return null;
  }
}

function ExplanationBody({ explanation, currency }: { explanation: MetricExplanation; currency?: string }) {
  const shown = valueText(explanation.value, currency);
  return (
    <div className="explain">
      <div className="explain-head">
        {shown !== null ? (
          // Only dollar figures blur under privacy mode, matching the cards —
          // a percentage or a day count is not sensitive the way a balance is.
          <div className={`explain-value num${explanation.value.kind === "money" ? " money" : ""}`}>{shown}</div>
        ) : (
          <div className="explain-value-withheld">Not shown yet</div>
        )}
        <p className="explain-def">{explanation.definition}</p>
      </div>

      {/* Withheld reasons lead — the "why there's no number" is the point. */}
      {explanation.warnings
        .filter((w) => w.level === "withheld")
        .map((w, i) => (
          <div key={`wh-${i}`} className="explain-warn withheld">
            <span className="explain-warn-ic" aria-hidden="true">–</span>
            <span>{w.message}</span>
          </div>
        ))}

      {explanation.inputs.length > 0 && (
        <section className="explain-sec">
          <div className="explain-sec-h">{shown !== null ? "Inputs" : "What would feed it"}</div>
          {explanation.inputs.map((input, i) => (
            <div key={i} className="explain-irow">
              <div className="explain-lbl">
                {input.label}
                {input.detail && <span className="explain-hint">{input.detail}</span>}
              </div>
              {input.amountCents !== null && (
                <div className={`explain-amt num money${input.amountCents < 0 ? " neg" : ""}`}>
                  {money(input.amountCents, currency ? { currency } : undefined)}
                </div>
              )}
            </div>
          ))}
        </section>
      )}

      <section className="explain-sec">
        <div className="explain-sec-h">Period</div>
        <div className="explain-chips">
          <span className="chip">{explanation.period}</span>
        </div>
      </section>

      {explanation.exclusions.length > 0 && (
        <section className="explain-sec">
          <div className="explain-sec-h">What&rsquo;s excluded</div>
          {explanation.exclusions.map((ex, i) => (
            <div key={i} className="explain-exline">
              <span className="explain-dot" aria-hidden="true">&bull;</span>
              <span>{ex}</span>
            </div>
          ))}
        </section>
      )}

      {explanation.assumptions.length > 0 && (
        <section className="explain-sec">
          <div className="explain-sec-h">Assumptions</div>
          {explanation.assumptions.map((a, i) => (
            <div key={i} className="explain-assume">
              <span className="explain-lbl">{a.label}</span>
              <span className="explain-aval">{a.value}</span>
            </div>
          ))}
        </section>
      )}

      {/* Non-withheld warnings (info / caution) close it out. */}
      {explanation.warnings
        .filter((w) => w.level !== "withheld")
        .map((w, i) => (
          <div key={`w-${i}`} className={`explain-warn ${w.level}`}>
            <span className="explain-warn-ic" aria-hidden="true">{w.level === "caution" ? "!" : "i"}</span>
            <span>{w.message}</span>
          </div>
        ))}
    </div>
  );
}
