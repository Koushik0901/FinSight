import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import type { NetWorthPoint } from "../api/client";
import { compactMoney } from "../utils/format";

const HEIGHT = 220;
const PAD_TOP = 34; // room for the end-value callout
const PAD_BOTTOM = 10;
const PAD_X = 8;
const GRID_LINES = 4;
const MAX_DOTS = 14; // beyond this many points, per-point markers just clutter

/**
 * Net-worth trendline. Drawn in real pixel space (container width measured
 * with a ResizeObserver) — never with `preserveAspectRatio="none"`, whose
 * non-uniform scaling smears strokes into thick blurry bands on wide windows.
 */
export default function NetWorthChart({ points, controls, rangeLabel = "6 months" }: { points: NetWorthPoint[]; controls?: ReactNode; rangeLabel?: string }) {
  const gradId = useId();
  const wrapRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    setWidth(el.clientWidth || 800);
    if (typeof ResizeObserver === "undefined") return; // jsdom
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width;
      if (w) setWidth(w);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  if (points.length < 2) {
    return <div className="stub">Net worth history is still building. Check back after a few days.</div>;
  }

  const w = width || 800;
  const values = points.map((p) => p.totalCents);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const innerW = w - PAD_X * 2;
  // Space points by their actual date, not evenly by index. Snapshots mix
  // monthly (backfilled history) and daily (going forward) cadences, so
  // index-spacing would stretch a 1-day gap as wide as a 30-day gap and bend
  // the trend. Time-proportional x keeps the slope honest.
  const times = points.map((p) => new Date(p.date).getTime());
  const tMin = times[0]!;
  const tMax = times[times.length - 1]!;
  const tSpan = tMax - tMin || 1;
  const xOf = (i: number) => PAD_X + ((times[i]! - tMin) / tSpan) * innerW;
  const yOf = (v: number) => PAD_TOP + (1 - (v - min) / range) * (HEIGHT - PAD_TOP - PAD_BOTTOM);

  const linePts = points.map((p, i) => ({ x: xOf(i), y: yOf(p.totalCents) }));
  const lineD = linePts.map((pt, i) => `${i === 0 ? "M" : "L"}${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(" ");
  const areaD = `${lineD} L${(PAD_X + innerW).toFixed(1)},${HEIGHT} L${PAD_X},${HEIGHT} Z`;
  const last = linePts[linePts.length - 1]!;
  const lastVal = values[values.length - 1]!;
  const lineColor = lastVal >= 0 ? "var(--accent)" : "var(--negative)";
  const showDots = points.length <= MAX_DOTS;

  // End-value callout, kept inside the frame.
  const label = compactMoney(lastVal);
  const labelY = Math.max(16, last.y - 16);
  const labelX = Math.min(last.x, w - 10);

  // Evenly thinned month labels (all when few points, ~6 otherwise).
  const labelEvery = Math.max(1, Math.ceil((points.length - 1) / 6));

  return (
    <div className="bigchart">
      <div className="bigchart-head">
        <div>
          <div className="h3">Net worth · last {rangeLabel}</div>
          <div className="muted" style={{ fontSize: 13, marginTop: 4 }}>Assets minus liabilities, marked monthly.</div>
        </div>
        {controls}
      </div>
      <div ref={wrapRef} style={{ width: "100%" }}>
        <svg viewBox={`0 0 ${w} ${HEIGHT}`} style={{ width: "100%", height: HEIGHT, display: "block" }} role="img" aria-label={`Net worth trend, currently ${label}`}>
          <defs>
            <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={lineColor} stopOpacity="0.28" />
              <stop offset="60%" stopColor={lineColor} stopOpacity="0.05" />
              <stop offset="100%" stopColor={lineColor} stopOpacity="0" />
            </linearGradient>
          </defs>
          {Array.from({ length: GRID_LINES }, (_, i) => {
            const y = PAD_TOP + ((i + 1) / (GRID_LINES + 1)) * (HEIGHT - PAD_TOP - PAD_BOTTOM);
            return <line key={i} x1={PAD_X} x2={w - PAD_X} y1={y} y2={y} stroke="var(--line)" strokeWidth="1" />;
          })}
          <path d={areaD} fill={`url(#${gradId})`} stroke="none" />
          <path d={lineD} fill="none" stroke={lineColor} strokeWidth="2" strokeLinejoin="round" strokeLinecap="round" />
          {showDots && linePts.slice(0, -1).map((pt, i) => (
            <circle key={points[i]!.date} cx={pt.x.toFixed(1)} cy={pt.y.toFixed(1)} r="3.5" fill="var(--elevated)" stroke={lineColor} strokeWidth="1.8" />
          ))}
          <circle cx={last.x.toFixed(1)} cy={last.y.toFixed(1)} r="6.5" fill={lineColor} stroke="var(--elevated)" strokeWidth="2.5" />
          <text
            x={labelX.toFixed(1)}
            y={labelY.toFixed(1)}
            textAnchor="end"
            fill={lineColor}
            style={{ font: "600 15px var(--sans, sans-serif)" }}
          >
            {label}
          </text>
        </svg>
      </div>
      <div style={{ position: "relative", height: 16, marginTop: 6 }}>
        {points.map((p, i) => {
          const show = i % labelEvery === 0 || i === points.length - 1;
          if (!show) return null;
          const isLast = i === points.length - 1;
          const isFirst = i === 0;
          return (
            <span
              key={p.date}
              style={{
                position: "absolute",
                left: `${(xOf(i) / w) * 100}%`,
                transform: isLast ? "translateX(-100%)" : isFirst ? "translateX(0)" : "translateX(-50%)",
                fontSize: 11,
                color: "var(--ink-faint)",
                fontFamily: "var(--mono)",
                whiteSpace: "nowrap",
              }}
            >
              {new Date(p.date).toLocaleDateString("en-US", { month: "short" })}
            </span>
          );
        })}
      </div>
    </div>
  );
}
