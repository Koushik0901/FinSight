import type { AccountBalancePoint } from "../api/client";

export default function AccountSparkline({
  points,
  color,
}: {
  points: AccountBalancePoint[];
  color: string;
}) {
  if (points.length < 2) {
    return (
      <svg viewBox="0 0 72 24" width="72" height="24" aria-hidden="true">
        <line
          x1="2"
          y1="12"
          x2="70"
          y2="12"
          stroke="var(--hairline)"
          strokeWidth="1"
          strokeDasharray="2 2"
        />
      </svg>
    );
  }

  const values = points.map((p) => p.balanceCents);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const stepX = 68 / (points.length - 1);
  const yOf = (v: number) => 21 - ((v - min) / range) * 18;

  const lineD = points
    .map((p, i) => `${i === 0 ? "M" : "L"}${(2 + i * stepX).toFixed(1)},${yOf(p.balanceCents).toFixed(1)}`)
    .join(" ");

  return (
    <svg viewBox="0 0 72 24" width="72" height="24" aria-hidden="true">
      <path d={lineD} fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}
