import Reveal from "../components/Reveal";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "../styles/reset.css";
import "../styles/tokens.css";
import "../styles/app.css";
import "../styles/mobile.css";
import "../styles/copilot-shell.css";
import NetWorthChart from "../components/NetWorthChart";
import { FinSightBarComparison } from "../components/copilot/charts/FinSightChart";
import CountUp from "../components/CountUp";
import type { NetWorthPoint } from "../api/openapiClient";
import { money } from "../utils/format";

const ptsA: NetWorthPoint[] = [
  { date: "2026-01-15", totalCents: 148_000_00 },
  { date: "2026-02-01", totalCents: 146_500_00 },
  { date: "2026-02-15", totalCents: 147_200_00 },
  { date: "2026-03-01", totalCents: 145_100_00 },
  { date: "2026-03-15", totalCents: 146_800_00 },
  { date: "2026-04-01", totalCents: 149_900_00 },
  { date: "2026-04-15", totalCents: 151_200_00 },
  { date: "2026-05-01", totalCents: 153_400_00 },
  { date: "2026-05-15", totalCents: 152_000_00 },
  { date: "2026-06-01", totalCents: 154_700_00 },
  { date: "2026-06-15", totalCents: 156_300_00 },
  { date: "2026-07-01", totalCents: 158_100_00 },
];

const ptsB: NetWorthPoint[] = [
  { date: "2026-01-15", totalCents: 148_000_00 },
  { date: "2026-02-01", totalCents: 140_000_00 },
  { date: "2026-02-15", totalCents: 138_500_00 },
  { date: "2026-03-01", totalCents: 136_000_00 },
  { date: "2026-03-15", totalCents: 137_800_00 },
  { date: "2026-04-01", totalCents: 135_200_00 },
  { date: "2026-04-15", totalCents: 133_900_00 },
  { date: "2026-05-01", totalCents: 131_400_00 },
  { date: "2026-05-15", totalCents: 130_000_00 },
  { date: "2026-06-01", totalCents: 128_600_00 },
  { date: "2026-06-15", totalCents: 126_900_00 },
  { date: "2026-07-01", totalCents: 124_100_00 },
];

function NetWorthDemo() {
  const [useB, setUseB] = useState(false);
  return (
    <div className="card" style={{ padding: 18 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
        <h3 style={{ margin: 0 }}>Net worth — draw-in replays on data switch</h3>
        <button id="nw-toggle" type="button" className="btn outline sm" onClick={() => setUseB((v) => !v)}>
          Switch dataset {useB ? "A" : "B"}
        </button>
      </div>
      <NetWorthChart points={useB ? ptsB : ptsA} rangeLabel="6 months" />
    </div>
  );
}

function CountUpDemo() {
  const [n, setN] = useState(0);
  useEffect(() => {
    const id = window.setInterval(() => setN((v) => (v + 1) % 4), 2600);
    return () => window.clearInterval(id);
  }, []);
  const values = [158_100_00, 124_100_00, 141_000_00, 165_500_00];
  const value = values[n]!;
  return (
    <div className="card" style={{ padding: 18 }}>
      <h3 style={{ margin: "0 0 8px" }}>CountUp — figure rolls on data change</h3>
      <div className="stat-row">
        <div className="stat">
          <div className="label">Net worth</div>
          <div className="value money" style={{ fontSize: 20 }}>
            <CountUp value={value} format={(v) => money(Math.round(v))} />
          </div>
        </div>
        <div className="stat">
          <div className="label">Savings rate</div>
          <div className="value" style={{ fontSize: 20 }}>
            <CountUp value={n * 11 + 4} format={(v) => `${Math.round(v)}%`} />
          </div>
        </div>
      </div>
      <p className="muted" style={{ fontSize: 12 }}>Cycles every 2.6s — capture right after a tick to see the roll.</p>
    </div>
  );
}

function BarsDemo() {
  const widths = [92, 64, 81, 47, 58, 33, 70];
  const heights = [120, 90, 140, 60, 104, 76, 128];
  return (
    <div className="card" style={{ padding: 18 }}>
      <h3 style={{ margin: "0 0 12px" }}>Shared primitives — grow-x rows, grow-y bars</h3>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {widths.map((wd, i) => (
          <div key={wd} style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <span className="mono muted" style={{ width: 40, fontSize: 11 }}>{wd}%</span>
            <div style={{ flex: 1, height: 12, background: "var(--surface-2)", borderRadius: 999 }}>
              <div className="plot-grow-x" style={{ width: `${wd}%`, height: "100%", background: "var(--accent)", borderRadius: 999, animationDelay: `${i * 60}ms` }} />
            </div>
          </div>
        ))}
      </div>
      <div style={{ display: "flex", gap: 12, alignItems: "end", height: 150, marginTop: 14 }}>
        {heights.map((h, i) => (
          <div key={h} style={{ flex: 1, display: "flex", alignItems: "end", height: "100%" }}>
            <div className="plot-grow-y" style={{ height: `${h}px`, width: "100%", background: "var(--accent)", borderRadius: 8, animationDelay: `${i * 60}ms` }} />
          </div>
        ))}
      </div>
    </div>
  );
}

function PlotVis() {
  return (
    <div style={{ padding: 28, maxWidth: 860, margin: "0 auto", display: "flex", flexDirection: "column", gap: 18, background: "var(--bg)", minHeight: "100vh" }}>
      <h2 style={{ margin: 0 }}>Plot motion visual check — scroll down, each plot reveals as it enters viewport</h2>
      <p className="muted" style={{ margin: 0 }}>This page is intentionally tall — scroll slowly to see each card animate only when visible. With <code>prefers-reduced-motion</code> everything shows instantly.</p>
      <div style={{ height: "40vh", display: "grid", placeItems: "center", border: "1px dashed var(--line)", borderRadius: 12, color: "var(--ink-faint)" }}>↓ scroll down ↓</div>
      <Reveal><NetWorthDemo /></Reveal>
      <Reveal><CountUpDemo /></Reveal>
      <Reveal><BarsDemo /></Reveal>
      <Reveal>
        <div className="card" style={{ padding: 18 }}>
          <h3 style={{ margin: "0 0 12px" }}>Recharts — FinSightBarComparison (mount draw, expo ease)</h3>
          <FinSightBarComparison current={{ label: "This month", amountCents: 482_300 }} prior={{ label: "Last month", amountCents: 311_700 }} title="Spending vs last month" />
        </div>
      </Reveal>
      <div style={{ height: "30vh" }} />
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<PlotVis />);
