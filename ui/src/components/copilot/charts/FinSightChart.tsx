import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Cell, ResponsiveContainer } from "recharts";
import { money } from "../../../utils/format";

export type MoneyPoint = { label: string; amountCents: number };

/**
 * Shared theming for every Recharts-based Copilot card: FinSight's own
 * typography/grid/axis colors via CSS variables, currency-formatted values,
 * and an explicit empty state — never raw Recharts defaults.
 *
 * NOTE on measured width: this reads its container's width via
 * ResponsiveContainer, which renders blank at width:0 and can re-animate on
 * every reflow. Any card using this component inside a still-streaming
 * message bubble must only mount it once streaming has finished — this is
 * handled by call sites in later tasks, not here.
 */
export function FinSightBarComparison({
  title,
  current,
  prior,
}: {
  title?: string;
  current: MoneyPoint;
  prior: MoneyPoint;
}) {
  if (current.amountCents === 0 && prior.amountCents === 0) {
    return (
      <div className="cp-card">
        {title && <p className="cp-card-title">{title}</p>}
        <p className="muted" style={{ fontSize: 12.5 }}>No comparison data available.</p>
      </div>
    );
  }

  const data = [
    { name: prior.label, value: prior.amountCents / 100, isCurrent: false },
    { name: current.label, value: current.amountCents / 100, isCurrent: true },
  ];

  return (
    <div className="cp-card">
      {title && <p className="cp-card-title" style={{ marginBottom: 12 }}>{title}</p>}
      <div style={{ width: "100%", height: 120 }}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data} layout="vertical" margin={{ top: 4, right: 24, bottom: 4, left: 4 }}>
            <CartesianGrid horizontal={false} stroke="var(--line)" />
            <XAxis
              type="number"
              tick={{ fill: "var(--ink-mute)", fontSize: 11 }}
              axisLine={{ stroke: "var(--line)" }}
              tickLine={false}
              tickFormatter={(v: number) => money(Math.round(v * 100))}
            />
            <YAxis
              type="category"
              dataKey="name"
              tick={{ fill: "var(--ink-2)", fontSize: 12 }}
              axisLine={false}
              tickLine={false}
              width={110}
            />
            <Bar dataKey="value" radius={[0, 5, 5, 0]} maxBarSize={22}>
              {data.map((entry) => (
                <Cell key={entry.name} fill={entry.isCurrent ? "var(--accent)" : "var(--ink-faint)"} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>
      <div className="row-sm" style={{ justifyContent: "space-between", marginTop: 4 }}>
        <span className="mono" style={{ fontSize: 12, color: "var(--ink-mute)" }}>
          <span>{prior.label}</span>: <span className="money">{money(prior.amountCents)}</span>
        </span>
        <span className="mono" style={{ fontSize: 12, color: "var(--ink)" }}>
          <span>{current.label}</span>: <span className="money">{money(current.amountCents)}</span>
        </span>
      </div>
    </div>
  );
}
