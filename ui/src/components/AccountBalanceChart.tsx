import type { AccountBalancePoint } from "../api/client";

export default function AccountBalanceChart({
  points,
  color,
}: {
  points: AccountBalancePoint[];
  color: string;
}) {
  if (points.length < 2) {
    return (
      <div style={{ height: 84, display: "grid", placeItems: "center" }}>
        <span className="muted" style={{ fontSize: 12 }}>
          Not enough history to show a trend.
        </span>
      </div>
    );
  }

  const values = points.map((p) => p.balanceCents);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const stepX = 100 / (points.length - 1);
  const yOf = (v: number) => 34 - ((v - min) / range) * 30;

  const linePts = points.map((p, i) => ({ x: i * stepX, y: yOf(p.balanceCents) }));
  const lineD = linePts
    .map((pt, i) => `${i === 0 ? "M" : "L"}${pt.x.toFixed(1)},${pt.y.toFixed(1)}`)
    .join(" ");
  const last = linePts[linePts.length - 1]!;

  return (
    <svg viewBox="0 0 100 40" preserveAspectRatio="none" style={{ width: "100%", height: 84, display: "block" }}>
      <path d={lineD} fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" />
      <circle cx={last.x.toFixed(1)} cy={last.y.toFixed(1)} r="2" fill={color} />
    </svg>
  );
}
