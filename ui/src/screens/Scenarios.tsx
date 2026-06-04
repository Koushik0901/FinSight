import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { commands, type ScenarioResult, type ScenarioParamsInput } from "../api/client";
import {
  useScenarioHistory,
  useRunScenario,
  useSaveScenario,
  useDeleteScenario,
} from "../api/hooks/useScenarios";
import * as I from "../components/Icons";

type Range = "6" | "12" | "24";

function fmt(cents: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(cents / 100);
}

function useDiningMonthly() {
  return useQuery<number>({
    queryKey: ["dining-monthly"],
    queryFn: async () => {
      const res = await commands.listCategoriesWithSpending();
      if (res.status === "error") throw new Error(res.error.message);
      const match = res.data.find((c) => /dining|restaurant|food|eat/i.test(c.label));
      return match?.thisMonthCents ?? 40000;
    },
    staleTime: 60_000,
  });
}

// ── Dual-line forecast chart ───────────────────────────────────────────────

function ForecastChart({
  baseline,
  scenario,
  range,
}: {
  baseline: number[];
  scenario: number[];
  range: Range;
}) {
  const count = range === "6" ? 6 : range === "24" ? 24 : 12;
  const base = baseline.slice(0, count);
  const scen = scenario.slice(0, count);
  const all = [...base, ...scen];
  const max = Math.max(...all, 1);
  const min = Math.min(...all, 0);
  const span = max - min || 1;
  const W = 100 / Math.max(base.length - 1, 1);
  const stressing = (scen[scen.length - 1] ?? 0) < (base[base.length - 1] ?? 0);
  const color = stressing ? "var(--negative)" : "var(--accent)";

  const path = (vals: number[]) =>
    vals
      .map((v, i) => {
        const x = i * W;
        const y = 38 - ((v - min) / span) * 34;
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");

  return (
    <div style={{ background: "var(--surface)", border: "1px solid var(--line)", borderRadius: "var(--radius-lg)", padding: "22px 8px 8px" }}>
      <svg viewBox="0 0 100 42" preserveAspectRatio="none" style={{ width: "100%", height: 200, display: "block" }}>
        <line x1="0" y1={(38 - ((0 - min) / span) * 34).toFixed(1)} x2="100" y2={(38 - ((0 - min) / span) * 34).toFixed(1)} stroke="var(--hairline)" strokeWidth="0.4" />
        <path d={path(base)} fill="none" stroke="var(--ink)" strokeWidth="1" />
        <path d={path(scen)} fill="none" stroke={color} strokeWidth="1.2" strokeDasharray="2.5 2" />
      </svg>
      <div style={{ display: "flex", gap: 16, fontSize: 12, color: "var(--ink-mute)", padding: "8px 12px 0" }}>
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span style={{ width: 14, height: 2, background: "var(--ink)", display: "inline-block" }} />current path
        </span>
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span style={{ width: 14, height: 2, background: color, display: "inline-block" }} />with scenario
        </span>
      </div>
    </div>
  );
}

// ── Results panel ──────────────────────────────────────────────────────────

function Results({
  description,
  result,
  onSaved,
  onDiscard,
}: {
  description: string;
  result: ScenarioResult;
  onSaved: () => void;
  onDiscard: () => void;
}) {
  const [range, setRange] = useState<Range>("12");
  const save = useSaveScenario();
  const coverable = result.verdict;

  return (
    <div style={{ marginTop: 24 }}>
      <div
        className="card"
        style={{
          borderColor: coverable ? "var(--accent)" : "var(--negative)",
        }}
      >
        <div className="screen-eyebrow" style={{ marginBottom: 10 }}>Verdict</div>
        <div style={{ fontSize: 22, fontWeight: 600, marginBottom: 6 }}>
          {coverable ? "You can do this — here's what changes." : "Not without trade-offs — here's what would give."}
        </div>
        <div className="muted" style={{ fontSize: 14 }}>&ldquo;{description}&rdquo;</div>

        <div className="stat-row" style={{ marginTop: 20 }}>
          <div className="stat">
            <div className="label">Runway change</div>
            <div className={`value figure ${result.runwayChangeDays >= 0 ? "" : "neg"}`}>
              {result.runwayChangeDays >= 0 ? "+" : ""}
              {result.runwayChangeDays} days
            </div>
          </div>
          <div className="stat">
            <div className="label">Monthly impact</div>
            <div className={`value figure money ${result.monthlyImpactCents >= 0 ? "" : "neg"}`}>
              {fmt(result.monthlyImpactCents)}
            </div>
          </div>
          <div className="stat">
            <div className="label">Goals affected</div>
            <div className="value figure">{result.goalsAffected.length}</div>
          </div>
        </div>
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 16 }}>
        <div className="toolbar">
          {(["6", "12", "24"] as Range[]).map((r) => (
            <button key={r} className={range === r ? "on" : ""} onClick={() => setRange(r)}>{r}M</button>
          ))}
        </div>
      </div>
      <div style={{ marginTop: 8 }}>
        <ForecastChart baseline={result.baselineMonthly} scenario={result.scenarioMonthly} range={range} />
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1.4fr 1fr", gap: 16, marginTop: 16 }}>
        <div className="card">
          <div className="screen-eyebrow" style={{ marginBottom: 12 }}>Worth knowing</div>
          <ol style={{ margin: 0, paddingLeft: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: 10 }}>
            {result.considerations.map((c, i) => (
              <li key={i} style={{ display: "grid", gridTemplateColumns: "22px 1fr", gap: 10, fontSize: 13.5, color: "var(--ink-2)", lineHeight: 1.5 }}>
                <span style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--ink-mute)" }}>{i + 1}</span>
                <span>{c}</span>
              </li>
            ))}
          </ol>
        </div>
        <div className="card" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div className="screen-eyebrow" style={{ marginBottom: 4 }}>What to do</div>
          <button
            className="btn primary"
            disabled={save.isPending}
            onClick={async () => {
              try {
                await save.mutateAsync({ description, result });
                toast.success("Scenario saved", { description });
                onSaved();
              } catch (e) {
                toast.error("Could not save scenario");
              }
            }}
          >
            <I.Sparkle /> Save this scenario
          </button>
          <button className="btn ghost" onClick={onDiscard}>
            <I.X /> Discard
          </button>
          <div className="muted" style={{ fontSize: 12, marginTop: 6, lineHeight: 1.5 }}>
            All scenarios are local — nothing happens to your real money until you explicitly apply changes.
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Main screen ────────────────────────────────────────────────────────────

export default function Scenarios() {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState<{ description: string; result: ScenarioResult } | null>(null);
  const run = useRunScenario();
  const del = useDeleteScenario();
  const { data: history = [] } = useScenarioHistory();
  const { data: diningMonthly = 40000 } = useDiningMonthly();

  const chips: { label: string; params: ScenarioParamsInput }[] = useMemo(
    () => [
      { label: "Cut income 50%", params: { incomeDeltaPct: -50, monthlyExpenseDeltaCents: 0, oneTimeCents: 0, startMonthOffset: 0, label: "Cut income 50%" } },
      { label: "Eliminate dining out", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: -diningMonthly, oneTimeCents: 0, startMonthOffset: 0, label: "Eliminate dining out" } },
      { label: "Buy a car $35k", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: 0, oneTimeCents: 3_500_000, startMonthOffset: 0, label: "Buy a car $35k" } },
      { label: "Add $500/mo to savings", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: 50_000, oneTimeCents: 0, startMonthOffset: 0, label: "Add $500/mo to savings" } },
    ],
    [diningMonthly]
  );

  const runWith = async (description: string, params: ScenarioParamsInput | null) => {
    try {
      const result = await run.mutateAsync({ description, months: 24, params });
      setActive({ description, result });
    } catch (e) {
      const code = (e as { code?: string }).code;
      if (code === "scenario.no_provider") {
        toast.error("Free-text needs an AI provider", {
          description: "Configure one in Settings, or pick a suggested scenario below.",
        });
      } else {
        toast.error("Could not run scenario", { description: (e as Error).message });
      }
    }
  };

  return (
    <div className="screen">
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">Scenarios · run any what-if</div>
          <h1>Imagine a future, see the math.</h1>
        </div>
      </div>

      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (query.trim()) void runWith(query.trim(), null);
        }}
        style={{ marginTop: 16 }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "16px 20px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: "var(--radius-lg)" }}>
          <I.Sparkle style={{ color: "var(--accent)" }} />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="What if I take a 6-month sabbatical?"
            aria-label="Scenario question"
            style={{ flex: 1, background: "transparent", border: 0, outline: 0, fontSize: 16, color: "var(--ink)" }}
          />
          <button type="submit" className="btn primary" disabled={run.isPending}>
            {run.isPending ? "Running…" : "Run"}
          </button>
        </div>
      </form>

      <div style={{ marginTop: 18 }}>
        <div className="screen-eyebrow" style={{ marginBottom: 10 }}>Or pick a starting point</div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {chips.map((c) => (
            <button key={c.label} className="chip" onClick={() => void runWith(c.label, c.params)}>
              {c.label}
            </button>
          ))}
        </div>
      </div>

      {active && (
        <Results
          description={active.description}
          result={active.result}
          onSaved={() => undefined}
          onDiscard={() => setActive(null)}
        />
      )}

      <div style={{ marginTop: 32 }}>
        <div className="screen-eyebrow" style={{ marginBottom: 10 }}>Recent scenarios you've run</div>
        <div className="card flush">
          {history.length === 0 ? (
            <div style={{ padding: 32, textAlign: "center", color: "var(--ink-faint)", fontSize: 13 }}>
              No scenarios saved. Run one above to keep it here.
            </div>
          ) : (
            history.map((h) => (
              <div key={h.id} style={{ display: "grid", gridTemplateColumns: "1fr auto auto auto", gap: 16, padding: "14px 20px", borderBottom: "1px solid var(--hairline)", alignItems: "center" }}>
                <div>
                  <div style={{ fontSize: 14 }}>{h.description}</div>
                  <span className={`chip ${h.result.verdict ? "positive" : "warning"}`} style={{ marginTop: 4 }}>
                    {h.result.verdict ? "Coverable" : "Not coverable"}
                  </span>
                </div>
                <span className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>
                  {new Date(h.createdAt).toLocaleDateString()}
                </span>
                <button className="btn ghost sm" onClick={() => setActive({ description: h.description, result: h.result })}>
                  Re-run
                </button>
                <button
                  className="btn ghost sm"
                  aria-label={`Delete ${h.description}`}
                  onClick={async () => {
                    try {
                      await del.mutateAsync(h.id);
                      toast("Scenario deleted");
                    } catch {
                      toast.error("Could not delete scenario");
                    }
                  }}
                >
                  <I.Trash />
                </button>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
