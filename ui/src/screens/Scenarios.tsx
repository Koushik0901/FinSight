import { useMemo, useState } from "react";
import { toast } from "sonner";
import { type ScenarioResult, type ScenarioParamsInput } from "../api/client";
import {
  useScenarioHistory,
  useRunScenario,
  useSaveScenario,
  useDeleteScenario,
} from "../api/hooks/useScenarios";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import * as I from "../components/Icons";
import Button from "../components/Button";
import Card from "../components/Card";
import Badge from "../components/Badge";
import EmptyState from "../components/EmptyState";
import { userErrorMessage } from "../utils/runtime";

type Range = "6" | "12" | "24";

function fmt(cents: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(cents / 100);
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
  const count = Number(range);
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
    <Card style={{ padding: "22px 8px 8px" }}>
      <svg viewBox="0 0 100 42" preserveAspectRatio="none" style={{ width: "100%", height: 200, display: "block" }}>
        <line x1="0" y1={(38 - ((0 - min) / span) * 34).toFixed(1)} x2="100" y2={(38 - ((0 - min) / span) * 34).toFixed(1)} stroke="var(--hairline)" strokeWidth="0.4" />
        <path d={path(base)} fill="none" stroke="var(--ink)" strokeWidth="1" />
        <path d={path(scen)} fill="none" stroke={color} strokeWidth="1.2" strokeDasharray="2.5 2" />
      </svg>
      <div className="row-md" style={{ fontSize: 12, color: "var(--ink-mute)", padding: "8px 12px 0" }}>
        <span className="row-xs">
          <span style={{ width: 14, height: 2, background: "var(--ink)", display: "inline-block" }} />current path
        </span>
        <span className="row-xs">
          <span style={{ width: 14, height: 2, background: color, display: "inline-block" }} />with scenario
        </span>
      </div>
    </Card>
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
    <div className="stack stack-lg" style={{ marginTop: 24 }}>
      <Card tone={coverable ? "accent" : "warn"} className="stack stack-md">
        <div className="screen-eyebrow">Verdict</div>
        <div style={{ fontSize: 22, fontWeight: 600 }}>
          {coverable ? "You can do this — here's what changes." : "Not without trade-offs — here's what would give."}
        </div>
        <div className="muted" style={{ fontSize: 14 }}>&ldquo;{description}&rdquo;</div>

        <div className="stat-row" style={{ marginTop: 12 }}>
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

        {result.goalsAffected.length > 0 && (
          <div className="stack stack-sm" style={{ marginTop: 4 }}>
            <div className="screen-eyebrow">Which goals</div>
            <ul className="stack stack-sm" style={{ margin: 0, paddingLeft: 0, listStyle: "none" }}>
              {result.goalsAffected.map((g, i) => (
                <li key={i} className="row-sm" style={{ fontSize: 13.5, color: "var(--ink-2)", lineHeight: 1.5, alignItems: "flex-start" }}>
                  <span>{g}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </Card>

      <div className="row" style={{ justifyContent: "flex-end" }}>
        <div className="toolbar">
          {(["6", "12", "24"] as Range[]).map((r) => (
            <button key={r} className={range === r ? "on" : ""} onClick={() => setRange(r)}>{r}M</button>
          ))}
        </div>
      </div>
      <ForecastChart baseline={result.baselineMonthly} scenario={result.scenarioMonthly} range={range} />

      <div className="responsive-grid" style={{ gridTemplateColumns: "1.4fr 1fr" }}>
        <Card className="stack stack-md">
          <div className="screen-eyebrow">Worth knowing</div>
          <ol className="stack stack-sm" style={{ margin: 0, paddingLeft: 0, listStyle: "none" }}>
            {result.considerations.map((c, i) => (
              <li key={i} className="row-sm" style={{ fontSize: 13.5, color: "var(--ink-2)", lineHeight: 1.5, alignItems: "flex-start" }}>
                <span className="num" style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--ink-mute)", width: 22, flexShrink: 0 }}>{i + 1}</span>
                <span>{c}</span>
              </li>
            ))}
          </ol>
        </Card>
        <Card className="stack stack-md">
          <div className="screen-eyebrow">What to do</div>
          <Button
            variant="primary"
            loading={save.isPending}
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
          </Button>
          <Button variant="ghost" onClick={onDiscard}>
            <I.X /> Discard
          </Button>
          <p className="muted" style={{ fontSize: 12, lineHeight: 1.5, margin: 0 }}>
            All scenarios are local — nothing happens to your real money until you explicitly apply changes.
          </p>
        </Card>
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
  const { data: categories } = useCategoriesWithSpending();
  const diningMonthly = useMemo(() => {
    const match = categories?.find((c) => /dining|restaurant|food|eat/i.test(c.label));
    if (!match) return 40000;
    return match.thisMonthCents > 0 ? match.thisMonthCents : match.lastMonthCents;
  }, [categories]);

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
        toast.error("Could not run scenario", {
          description: userErrorMessage(e, "Try again from the desktop app after your data loads."),
        });
      }
    }
  };

  return (
    <div className="screen screen-scenarios">
      <header className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Scenarios · what-if</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Play out the possibilities.</h1>
          <div className="muted" style={{ marginTop: 6 }}>Imagine a future, see the math.</div>
        </div>
      </header>

      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (query.trim()) void runWith(query.trim(), null);
        }}
        style={{ marginTop: 16 }}
      >
        <div className="scenario-composer">
          <I.Sparkle style={{ color: "var(--accent)" }} />
          <input
            className="scenario-input"
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="What if I take a 6-month sabbatical?"
            aria-label="Scenario question"
          />
          <Button type="submit" variant="primary" disabled={run.isPending}>
            {run.isPending ? "Running…" : "Run"}
          </Button>
        </div>
      </form>

      <div className="stack stack-sm" style={{ marginTop: 18 }}>
        <div className="screen-eyebrow">Or pick a starting point</div>
        <div className="row-sm wrap">
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

      <section className="stack stack-md" style={{ marginTop: 32 }}>
        <div className="screen-eyebrow">Recent scenarios you've run</div>
        <Card flush>
          {history.length === 0 ? (
            <EmptyState
              compact
              icon={<I.Sparkle style={{ color: "var(--ink-faint)", width: 24, height: 24 }} />}
              title="No scenarios saved"
              description="Run one above to keep it here."
            />
          ) : (
            <ul className="stack" style={{ margin: 0, padding: 0, listStyle: "none" }}>
              {history.map((h, i) => (
                <li
                  key={h.id}
                  className="row-md"
                  style={{
                    padding: "14px 20px",
                    borderBottom: i < history.length - 1 ? "1px solid var(--hairline)" : "none",
                    alignItems: "center",
                  }}
                >
                  <div className="grow stack stack-xs">
                    <div style={{ fontSize: 14 }}>{h.description}</div>
                    <Badge tone={h.result.verdict ? "positive" : "warning"}>
                      {h.result.verdict ? "Coverable" : "Not coverable"}
                    </Badge>
                  </div>
                  <span className="num muted" style={{ fontSize: 11.5, whiteSpace: "nowrap" }}>
                    {new Date(h.createdAt).toLocaleDateString()}
                  </span>
                  <Button variant="ghost" size="sm" onClick={() => setActive({ description: h.description, result: h.result })}>
                    View
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
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
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </section>
    </div>
  );
}
