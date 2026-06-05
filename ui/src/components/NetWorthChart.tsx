import { useId } from "react";
import type { NetWorthPoint } from "../api/client";

function fmt(cents: number) {
  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(cents / 100);
}

export default function NetWorthChart({ points }: { points: NetWorthPoint[] }) {
  const gradId = useId();

  if (points.length < 2) {
    return <div className="stub">Net worth history is still building. Check back after a few days.</div>;
  }

  const values = points.map((p) => p.totalCents);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const stepX = 100 / (points.length - 1);

  // y maps value→[34 (bottom) .. 4 (top)] within a 0..40 viewBox.
  const yOf = (v: number) => 34 - ((v - min) / range) * 30;

  const linePts = points.map((p, i) => ({ x: i * stepX, y: yOf(p.totalCents) }));
  const lineD = linePts.map((pt, i) => `${i === 0 ? "M" : "L"}${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(" ");
  const areaD = `${lineD} L100,40 L0,40 Z`;
  const last = linePts[linePts.length - 1]!;
  const lastVal = values[values.length - 1]!;

  return (
    <div style={{ background: "var(--surface)", border: "1px solid var(--line)", borderRadius: "var(--radius-lg)", padding: "20px 4px 12px" }}>
      <div style={{ padding: "0 18px 12px" }}>
        <div className="eyebrow">Net worth</div>
        <div className="figure money num" style={{ fontSize: 24, marginTop: 4 }}>{fmt(lastVal)}</div>
      </div>
      <svg viewBox="0 0 100 40" preserveAspectRatio="none" style={{ width: "100%", height: 140, display: "block" }}>
        <defs>
          <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.34" />
            <stop offset="60%" stopColor="var(--accent)" stopOpacity="0.06" />
            <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <path d={areaD} fill={`url(#${gradId})`} stroke="none" />
        <path d={lineD} fill="none" stroke="var(--accent)" strokeWidth="1.2" />
        <circle cx={last.x.toFixed(1)} cy={last.y.toFixed(1)} r="1.6" fill="var(--accent)" />
      </svg>
      <div style={{ display: "flex", padding: "4px 4px 0", justifyContent: "space-between" }}>
        {points.map((p, i) => (
          (i === 0 || i === points.length - 1 || i === Math.floor(points.length / 2)) ? (
            <span key={p.date} style={{ fontSize: 11, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>
              {new Date(p.date).toLocaleDateString("en-US", { month: "short" })}
            </span>
          ) : null
        ))}
      </div>
    </div>
  );
}
