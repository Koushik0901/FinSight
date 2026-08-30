import { useMemo, useState } from "react";
import { toast } from "sonner";
import {
  type SavedScenarioDetail,
  type ScenarioParamsInput,
  type ScenarioPlanProposal,
} from "../../api/openapiClient";
import {
  useSavedScenarios,
  useRunScenario,
  useSaveScenario,
  useDuplicateScenario,
  useArchiveScenario,
  usePromoteScenario,
  useApplyScenario,
  useReviseScenario,
  useClearScenarioRevision,
  useDeleteScenario,
  useScenarioExplanation,
} from "../../api/hooks/useScenarios";
import { money } from "../../utils/format";
import { userErrorMessage } from "../../utils/runtime";
import * as I from "../../components/Icons";
import { MobilePageHeader } from "../../components/mobile/MobilePageHeader";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { MobileEmptyState } from "../../components/mobile/MobileEmptyState";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import { StickyActionBar } from "../../components/mobile/StickyActionBar";

const fmt = (cents: number) => money(cents);

type Filter = "all" | "affordable" | "at-risk" | "stale";

function impactSubtitle(s: SavedScenarioDetail): string {
  const shown = s.currentResult ?? s.originalResult;
  const verdict = shown.verdict ? "Stays afloat" : "At risk";
  const runway = `${shown.runwayChangeDays >= 0 ? "+" : ""}${shown.runwayChangeDays}d runway`;
  const stale = s.isStale ? " · stale — finances changed" : "";
  const revised = s.revisedParams ? " · revised" : "";
  const legacy = !s.recomputable ? " · legacy" : "";
  return `${verdict} · ${runway}${stale}${revised}${legacy}`;
}

function verdictTone(s: SavedScenarioDetail): "positive" | "negative" {
  const shown = s.currentResult ?? s.originalResult;
  return shown.verdict ? "positive" : "negative";
}

function dotColor(s: SavedScenarioDetail): string {
  const shown = s.currentResult ?? s.originalResult;
  if (s.isStale) return "var(--warning)";
  return shown.verdict ? "var(--positive)" : "var(--negative)";
}

export default function MobileScenarios() {
  const { data: saved = [], isLoading } = useSavedScenarios();
  const run = useRunScenario();
  const save = useSaveScenario();
  const dup = useDuplicateScenario();
  const archive = useArchiveScenario();
  const promote = usePromoteScenario();
  const del = useDeleteScenario();
  const revise = useReviseScenario();
  const clearRev = useClearScenarioRevision();
  const apply = useApplyScenario();

  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [active, setActive] = useState<{ description: string; result: SavedScenarioDetail["originalResult"]; params: ScenarioParamsInput; months: number } | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [reviseMode, setReviseMode] = useState(false);
  const [incomePct, setIncomePct] = useState(0);
  const [expenseDollars, setExpenseDollars] = useState(0);
  const [oneTimeDollars, setOneTimeDollars] = useState(0);
  const [proposal, setProposal] = useState<ScenarioPlanProposal | null>(null);
  const [approved, setApproved] = useState<Set<string>>(new Set());
  const [explainOpen, setExplainOpen] = useState(false);
  const selected = useMemo(() => (selectedId ? saved.find((x) => x.id === selectedId) ?? null : null), [saved, selectedId]);
  const { data: explanation, isLoading: explaining } = useScenarioExplanation(explainOpen && selected ? selected.id : null);

  const chips: { label: string; params: ScenarioParamsInput }[] = useMemo(
    () => [
      { label: "Cut income 50%", params: { incomeDeltaPct: -50, monthlyExpenseDeltaCents: 0, oneTimeCents: 0, startMonthOffset: 0, label: "Cut income 50%" } },
      { label: "Buy a car $35k", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: 0, oneTimeCents: 3_500_000, startMonthOffset: 0, label: "Buy a car $35k" } },
      { label: "Add $500/mo savings", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: 50_000, oneTimeCents: 0, startMonthOffset: 0, label: "Add $500/mo savings" } },
      { label: "Cut $400/mo spend", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: -40_000, oneTimeCents: 0, startMonthOffset: 0, label: "Cut $400/mo spend" } },
    ],
    [],
  );

  const filtered = useMemo(() => {
    if (filter === "affordable") return saved.filter((s) => (s.currentResult ?? s.originalResult).verdict);
    if (filter === "at-risk") return saved.filter((s) => !(s.currentResult ?? s.originalResult).verdict);
    if (filter === "stale") return saved.filter((s) => s.isStale);
    return saved;
  }, [saved, filter]);

  const openDetail = (s: SavedScenarioDetail) => {
    setSelectedId(s.id);
    setReviseMode(false);
    setProposal(null);
    setExplainOpen(false);
    const base = s.revisedParams ?? s.params;
    setIncomePct(base?.incomeDeltaPct ?? 0);
    setExpenseDollars((base?.monthlyExpenseDeltaCents ?? 0) / 100);
    setOneTimeDollars((base?.oneTimeCents ?? 0) / 100);
  };

  const closeDetail = () => {
    setSelectedId(null);
    setReviseMode(false);
    setProposal(null);
    setExplainOpen(false);
  };

  const runWith = async (description: string, params: ScenarioParamsInput | null) => {
    try {
      const ran = await run.mutateAsync({ description, months: 24, params });
      setActive({ description, result: ran.result, params: ran.params, months: ran.months });
    } catch (e) {
      const code = (e as { code?: string }).code;
      if (code === "scenario.no_provider") {
        toast.error("Free-text needs an AI provider", { description: "Configure one in Settings, or pick a suggested scenario below." });
      } else {
        toast.error("Could not run scenario", { description: userErrorMessage(e, "Check connection and try again.") });
      }
    }
  };

  if (isLoading) {
    return (
      <div className="stub" style={{ padding: 16 }}>
        <span className="spinner" aria-hidden="true" /> Loading scenarios…
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: "calc(24px + env(safe-area-inset-bottom, 0px))" }}>
      <MobilePageHeader
        title="Scenarios"
        description="What-if explorer — play out a future, see the math. Tap any saved run for inputs, impact, and next steps."
      />

      {/* Composer — thumb-reach, 44px targets */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (query.trim()) void runWith(query.trim(), null);
          }}
          style={{ display: "flex", gap: 8, alignItems: "center" }}
        >
          <div
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              gap: 8,
              minHeight: 44,
              padding: "0 12px",
              borderRadius: 12,
              border: "1px solid var(--line)",
              background: "var(--surface)",
            }}
          >
            <span style={{ color: "var(--accent)", display: "inline-flex" }} aria-hidden="true">
              <I.Sparkle width={16} height={16} />
            </span>
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="What if I take 6 months off?"
              aria-label="Scenario question"
              style={{
                flex: 1,
                border: "none",
                outline: "none",
                background: "transparent",
                fontSize: 15,
                color: "var(--ink)",
                minWidth: 0,
              }}
            />
          </div>
          <button type="submit" className="btn primary" style={{ minHeight: 44, minWidth: 72, flexShrink: 0 }} disabled={run.isPending}>
            {run.isPending ? "…" : "Run"}
          </button>
        </form>

        <div style={{ display: "flex", gap: 8, overflowX: "auto", paddingBottom: 2, scrollbarWidth: "none" }}>
          {chips.map((c) => (
            <button
              key={c.label}
              type="button"
              className="chip"
              onClick={() => void runWith(c.label, c.params)}
              style={{ whiteSpace: "nowrap", minHeight: 32, flexShrink: 0 }}
            >
              {c.label}
            </button>
          ))}
        </div>

        {active ? (
          <div style={{ padding: 14, border: "1px solid var(--line)", borderRadius: 16, background: active.result.verdict ? "color-mix(in oklab, var(--positive) 8%, var(--surface))" : "color-mix(in oklab, var(--warning) 10%, var(--surface))", display: "flex", flexDirection: "column", gap: 10 }}>
            <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 700 }}>Result · “{active.description}”</div>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
              <div style={{ padding: 10, borderRadius: 12, background: "var(--surface)", border: "1px solid var(--line)" }}>
                <div style={{ fontSize: 11, color: "var(--ink-faint)", fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase" }}>Runway change</div>
                <div className="num" style={{ fontWeight: 750, marginTop: 4, color: active.result.runwayChangeDays >= 0 ? "var(--positive)" : "var(--negative)" }}>
                  {active.result.runwayChangeDays >= 0 ? "+" : ""}
                  {active.result.runwayChangeDays} days
                </div>
              </div>
              <div style={{ padding: 10, borderRadius: 12, background: "var(--surface)", border: "1px solid var(--line)" }}>
                <div style={{ fontSize: 11, color: "var(--ink-faint)", fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase" }}>Monthly impact</div>
                <div className="money num" style={{ fontWeight: 750, marginTop: 4 }}>
                  {fmt(active.result.monthlyImpactCents)}
                </div>
              </div>
            </div>
            {active.result.considerations.length > 0 ? (
              <ul style={{ margin: 0, paddingLeft: 16, fontSize: 13, lineHeight: 1.5, color: "var(--ink-2)" }}>
                {active.result.considerations.slice(0, 2).map((c, i) => (
                  <li key={i}>{c}</li>
                ))}
              </ul>
            ) : null}
            <div style={{ display: "flex", gap: 8 }}>
              <button
                type="button"
                className="btn primary"
                style={{ flex: 1, minHeight: 44 }}
                disabled={save.isPending}
                onClick={async () => {
                  try {
                    await save.mutateAsync({ description: active.description, params: active.params, months: active.months });
                    toast.success("Scenario saved");
                    setActive(null);
                  } catch {
                    toast.error("Could not save scenario");
                  }
                }}
              >
                <I.Sparkle width={14} height={14} /> Save
              </button>
              <button type="button" className="btn" style={{ flex: 1, minHeight: 44 }} onClick={() => setActive(null)}>
                Discard
              </button>
            </div>
          </div>
        ) : null}
      </div>

      {/* Scenario type filter — chips */}
      <div style={{ display: "flex", gap: 8, overflowX: "auto", scrollbarWidth: "none", paddingBottom: 2 }} role="group" aria-label="Scenario filter">
        {(["all", "affordable", "at-risk", "stale"] as Filter[]).map((f) => {
          const on = filter === f;
          const label = f === "all" ? "All" : f === "affordable" ? "Affordable" : f === "at-risk" ? "At risk" : "Stale";
          return (
            <button
              key={f}
              type="button"
              onClick={() => setFilter(f)}
              aria-pressed={on}
              className="chip"
              style={{
                whiteSpace: "nowrap",
                minHeight: 36,
                flexShrink: 0,
                background: on ? "var(--ink)" : "var(--surface)",
                color: on ? "var(--elevated)" : "var(--ink)",
                borderColor: on ? "var(--ink)" : "var(--line)",
                fontWeight: on ? 650 : 500,
              }}
            >
              {label}
              {f !== "all" ? (
                <span style={{ marginLeft: 6, fontSize: 12, opacity: 0.85 }}>
                  {f === "affordable"
                    ? saved.filter((s) => (s.currentResult ?? s.originalResult).verdict).length
                    : f === "at-risk"
                      ? saved.filter((s) => !(s.currentResult ?? s.originalResult).verdict).length
                      : saved.filter((s) => s.isStale).length}
                </span>
              ) : null}
            </button>
          );
        })}
      </div>

      {/* List — mobile cards */}
      {saved.length === 0 ? (
        <MobileEmptyState
          icon={<I.Sparkle width={28} height={28} />}
          title="No saved scenarios yet"
          description="Run a what-if above and save it to compare, revise, and promote to your plan — nothing touches real money until you apply."
        />
      ) : filtered.length === 0 ? (
        <MobileEmptyState
          icon={<I.Info width={28} height={28} />}
          title={`No ${filter} scenarios`}
          description="Try a different filter or run a new what-if above."
        />
      ) : (
        <MobileSection title="Saved scenarios" description="One insight at a time — tap a card for inputs, impact, and actions. Each is recomputed against today so the list compares fairly.">
          <MobileList ariaLabel="Saved scenarios">
            {filtered.map((s) => {
              const shown = s.currentResult ?? s.originalResult;
              const tone = verdictTone(s);
              return (
                <MobileListItem
                  key={s.id}
                  icon={<span style={{ width: 10, height: 10, borderRadius: "50%", background: dotColor(s), display: "inline-block", flexShrink: 0 }} aria-hidden="true" />}
                  title={s.description}
                  subtitle={impactSubtitle(s)}
                  value={fmt(shown.monthlyImpactCents)}
                  valueTone={shown.monthlyImpactCents >= 0 ? "default" : "negative"}
                  meta={`${shown.verdict ? "Yes" : "At risk"} · ${shown.goalsAffected.length ? `${shown.goalsAffected.length} goals` : "no goals flagged"}`}
                  onPress={() => openDetail(s)}
                  rightExtra={
                    <span
                      className={`chip ${tone === "positive" ? "positive" : "warning"}`}
                      style={{ fontSize: 11, padding: "2px 8px", minHeight: 22, lineHeight: "18px" }}
                    >
                      {tone === "positive" ? "Yes" : "At risk"}
                    </span>
                  }
                />
              );
            })}
          </MobileList>
        </MobileSection>
      )}

      {/* Detail — BottomSheet (not dialog) */}
      <BottomSheet open={!!selected} onClose={closeDetail} title={selected?.description ?? "Scenario"}>
        {selected ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 16, paddingBottom: 8 }}>
            {/* Hero */}
            {(() => {
              const shown = selected.currentResult ?? selected.originalResult;
              const stale = selected.isStale === true;
              const runwayDrifted = selected.currentResult && selected.currentResult.runwayChangeDays !== selected.originalResult.runwayChangeDays;
              return (
                <>
                  <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
                    <span className={`chip ${shown.verdict ? "positive" : "warning"}`} style={{ fontWeight: 650 }}>
                      {shown.verdict ? "Stays afloat" : "At risk"}
                    </span>
                    {stale ? <span className="chip warning">Stale · finances changed</span> : null}
                    {selected.revisedParams ? <span className="chip accent">Revised</span> : null}
                    {!selected.recomputable ? <span className="chip">Legacy</span> : null}
                    {selected.revisedResult ? <span className="chip accent">Has revision</span> : null}
                  </div>

                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
                    <div style={{ padding: 12, borderRadius: 12, background: "var(--surface)", border: "1px solid var(--line)" }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 700 }}>Runway change</div>
                      <div className="num" style={{ fontSize: 20, fontWeight: 800, marginTop: 4, color: shown.runwayChangeDays >= 0 ? "var(--positive)" : "var(--negative)" }}>
                        {shown.runwayChangeDays >= 0 ? "+" : ""}
                        {shown.runwayChangeDays}d
                      </div>
                      {stale && runwayDrifted ? (
                        <div style={{ fontSize: 11, color: "var(--ink-mute)", marginTop: 4 }}>
                          was {selected.originalResult.runwayChangeDays >= 0 ? "+" : ""}
                          {selected.originalResult.runwayChangeDays}d
                        </div>
                      ) : null}
                    </div>
                    <div style={{ padding: 12, borderRadius: 12, background: "var(--surface)", border: "1px solid var(--line)" }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 700 }}>Monthly impact</div>
                      <div className="money num" style={{ fontSize: 18, fontWeight: 800, marginTop: 4, letterSpacing: "-0.02em" }}>
                        {fmt(shown.monthlyImpactCents)}
                      </div>
                      <div style={{ fontSize: 11, color: "var(--ink-mute)", marginTop: 4 }}>{shown.goalsAffected.length ? `${shown.goalsAffected.length} goals flagged` : "No goals flagged"}</div>
                    </div>
                  </div>

                  <div style={{ fontSize: 12, color: "var(--ink-mute)", lineHeight: 1.5 }}>
                    Saved {new Date(selected.createdAt).toLocaleDateString()} · {selected.months} months · {selected.recomputable ? "recomputed against today" : "original only"}
                  </div>
                </>
              );
            })()}

            {/* Inputs — progressive disclosure */}
            <div style={{ padding: 12, borderRadius: 12, border: "1px solid var(--line)", background: "var(--surface)", display: "flex", flexDirection: "column", gap: 12 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
                <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 700 }}>Scenario inputs</div>
                <button
                  type="button"
                  className="btn"
                  style={{ minHeight: 32, padding: "0 12px", fontSize: 13 }}
                  onClick={() => {
                    if (!reviseMode) {
                      const base = selected.revisedParams ?? selected.params;
                      setIncomePct(base?.incomeDeltaPct ?? 0);
                      setExpenseDollars((base?.monthlyExpenseDeltaCents ?? 0) / 100);
                      setOneTimeDollars((base?.oneTimeCents ?? 0) / 100);
                    }
                    setReviseMode((v) => !v);
                  }}
                >
                  {reviseMode ? "Cancel" : "Edit"}
                </button>
              </div>

              {!reviseMode ? (
                <div style={{ display: "grid", gap: 8, fontSize: 13, lineHeight: 1.5 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "10px 12px", borderRadius: 10, background: "var(--surface-2)", border: "1px solid var(--line)" }}>
                    <span style={{ color: "var(--ink-mute)" }}>Income change</span>
                    <span className="num" style={{ fontWeight: 650 }}>{(selected.revisedParams ?? selected.params)?.incomeDeltaPct ?? 0}%</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "10px 12px", borderRadius: 10, background: "var(--surface-2)", border: "1px solid var(--line)" }}>
                    <span style={{ color: "var(--ink-mute)" }}>Monthly spend change</span>
                    <span className="money num" style={{ fontWeight: 650 }}>{fmt((selected.revisedParams ?? selected.params)?.monthlyExpenseDeltaCents ?? 0)}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "10px 12px", borderRadius: 10, background: "var(--surface-2)", border: "1px solid var(--line)" }}>
                    <span style={{ color: "var(--ink-mute)" }}>One-time amount</span>
                    <span className="money num" style={{ fontWeight: 650 }}>{fmt((selected.revisedParams ?? selected.params)?.oneTimeCents ?? 0)}</span>
                  </div>
                </div>
              ) : (
                <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                  <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <span style={{ fontSize: 12, color: "var(--ink-mute)", fontWeight: 600 }}>Income change (%)</span>
                    <input className="control" type="number" value={incomePct} onChange={(e) => setIncomePct(Number(e.target.value))} style={{ minHeight: 44, borderRadius: 10, padding: "0 12px", border: "1px solid var(--line)", background: "var(--elevated)", color: "var(--ink)" }} />
                  </label>
                  <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <span style={{ fontSize: 12, color: "var(--ink-mute)", fontWeight: 600 }}>Monthly spend change ($)</span>
                    <input className="control" type="number" value={expenseDollars} onChange={(e) => setExpenseDollars(Number(e.target.value))} style={{ minHeight: 44, borderRadius: 10, padding: "0 12px", border: "1px solid var(--line)", background: "var(--elevated)", color: "var(--ink)" }} />
                  </label>
                  <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <span style={{ fontSize: 12, color: "var(--ink-mute)", fontWeight: 600 }}>One-time amount ($)</span>
                    <input className="control" type="number" value={oneTimeDollars} onChange={(e) => setOneTimeDollars(Number(e.target.value))} style={{ minHeight: 44, borderRadius: 10, padding: "0 12px", border: "1px solid var(--line)", background: "var(--elevated)", color: "var(--ink)" }} />
                  </label>
                  <button
                    type="button"
                    className="btn primary"
                    style={{ minHeight: 44 }}
                    disabled={revise.isPending || clearRev.isPending}
                    onClick={async () => {
                      const base = selected.revisedParams ?? selected.params;
                      try {
                        await revise.mutateAsync({
                          id: selected.id,
                          params: {
                            incomeDeltaPct: Math.round(incomePct),
                            monthlyExpenseDeltaCents: Math.round(expenseDollars * 100),
                            oneTimeCents: Math.round(oneTimeDollars * 100),
                            startMonthOffset: base?.startMonthOffset ?? 0,
                            label: base?.label ?? selected.description,
                          },
                        });
                        toast.success("Scenario re-evaluated");
                        setReviseMode(false);
                      } catch (e) {
                        toast.error("Could not revise scenario", { description: userErrorMessage(e, "Try again.") });
                      }
                    }}
                  >
                    {revise.isPending ? "Re-evaluating…" : "Re-evaluate"}
                  </button>
                  {selected.revisedParams ? (
                    <button
                      type="button"
                      className="btn"
                      style={{ minHeight: 44 }}
                      disabled={clearRev.isPending}
                      onClick={async () => {
                        try {
                          await clearRev.mutateAsync(selected.id);
                          toast("Revision discarded");
                          setReviseMode(false);
                        } catch {
                          toast.error("Could not discard the revision");
                        }
                      }}
                    >
                      Discard revision
                    </button>
                  ) : null}
                  {selected.revisedResult ? (
                    <div style={{ padding: 10, borderRadius: 10, background: "var(--surface-2)", border: "1px solid var(--line)", fontSize: 12, lineHeight: 1.5, color: "var(--ink-mute)" }}>
                      <div style={{ fontWeight: 700, color: "var(--ink)", marginBottom: 4 }}>Revised vs original</div>
                      <div>Verdict: {selected.revisedResult.verdict ? "Yes" : "At risk"} vs {(selected.currentResult ?? selected.originalResult).verdict ? "Yes" : "At risk"}</div>
                      <div>Runway: {selected.revisedResult.runwayChangeDays >= 0 ? "+" : ""}{selected.revisedResult.runwayChangeDays}d vs {(selected.currentResult ?? selected.originalResult).runwayChangeDays >= 0 ? "+" : ""}{(selected.currentResult ?? selected.originalResult).runwayChangeDays}d</div>
                      <div className="money">Impact: {fmt(selected.revisedResult.monthlyImpactCents)} vs {fmt((selected.currentResult ?? selected.originalResult).monthlyImpactCents)}</div>
                    </div>
                  ) : null}
                </div>
              )}
            </div>

            {/* Considerations & goals — one insight at a time, full width */}
            {(() => {
              const shown = selected.currentResult ?? selected.originalResult;
              return (
                <>
                  {shown.considerations.length > 0 ? (
                    <div style={{ padding: 12, borderRadius: 12, background: "var(--surface-2)", border: "1px solid var(--line)" }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 700, marginBottom: 8 }}>Worth knowing</div>
                      <ol style={{ margin: 0, paddingLeft: 18, display: "flex", flexDirection: "column", gap: 6 }}>
                        {shown.considerations.map((c, i) => (
                          <li key={i} style={{ fontSize: 13, color: "var(--ink-2)", lineHeight: 1.5 }}>{c}</li>
                        ))}
                      </ol>
                    </div>
                  ) : null}
                  {shown.goalsAffected.length > 0 ? (
                    <div style={{ padding: 12, borderRadius: 12, background: "var(--surface)", border: "1px solid var(--line)" }}>
                      <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 700, marginBottom: 8 }}>Goals affected</div>
                      <ul style={{ margin: 0, paddingLeft: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: 6 }}>
                        {shown.goalsAffected.map((g, i) => (
                          <li key={i} style={{ fontSize: 13, color: "var(--ink-2)", lineHeight: 1.45, display: "flex", gap: 8, alignItems: "flex-start" }}>
                            <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--accent)", marginTop: 7, flexShrink: 0 }} />
                            <span>{g}</span>
                          </li>
                        ))}
                      </ul>
                    </div>
                  ) : null}
                </>
              );
            })()}

            {/* Explain */}
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <button
                type="button"
                className="btn"
                style={{ minHeight: 44, flex: 1 }}
                onClick={() => setExplainOpen((v) => !v)}
                aria-expanded={explainOpen}
              >
                <I.Info width={14} height={14} /> {explainOpen ? "Hide explanation" : "Explain"}
              </button>
            </div>
            {explainOpen ? (
              <div style={{ padding: 12, borderRadius: 12, border: "1px solid var(--line)", background: "var(--surface-2)", fontSize: 13, lineHeight: 1.5, color: "var(--ink-2)" }}>
                {explaining ? (
                  <div style={{ display: "flex", alignItems: "center", gap: 8, color: "var(--ink-mute)" }}>
                    <span className="spinner" aria-hidden="true" /> Explaining…
                  </div>
                ) : explanation ? (
                  <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                    {explanation.inputs?.length ? (
                      <div>
                        <div style={{ fontWeight: 700, color: "var(--ink)", fontSize: 12 }}>Inputs</div>
                        <ul style={{ margin: "4px 0 0", paddingLeft: 16 }}>
                          {explanation.inputs.map((x, i) => (
                            <li key={i}>{typeof x === "string" ? x : (x as { label?: string }).label ?? String(x)}</li>
                          ))}
                        </ul>
                      </div>
                    ) : null}
                    {explanation.assumptions?.length ? (
                      <div>
                        <div style={{ fontWeight: 700, color: "var(--ink)", fontSize: 12 }}>Assumptions</div>
                        <ul style={{ margin: "4px 0 0", paddingLeft: 16 }}>
                          {explanation.assumptions.map((x, i) => (
                            <li key={i}>{typeof x === "string" ? x : (x as { label?: string }).label ?? String(x)}</li>
                          ))}
                        </ul>
                      </div>
                    ) : null}
                    {explanation.warnings?.length ? (
                      <div>
                        <div style={{ fontWeight: 700, color: "var(--negative)", fontSize: 12 }}>Warnings</div>
                        <ul style={{ margin: "4px 0 0", paddingLeft: 16 }}>
                          {explanation.warnings.map((x, i) => (
                            <li key={i}>{typeof x === "string" ? x : (x as { message?: string }).message ?? String(x)}</li>
                          ))}
                        </ul>
                      </div>
                    ) : null}
                    {!explanation.inputs?.length && !explanation.assumptions?.length && !explanation.warnings?.length ? (
                      <div style={{ color: "var(--ink-mute)" }}>{(explanation as unknown as { note?: string }).note ?? "No explanation available for this scenario."}</div>
                    ) : null}
                  </div>
                ) : (
                  <div style={{ color: "var(--ink-mute)" }}>No explanation available.</div>
                )}
              </div>
            ) : null}

            {/* Promote proposal */}
            {proposal ? (
              <div style={{ padding: 12, borderRadius: 12, border: "1px solid var(--hairline)", background: "var(--surface)", display: "flex", flexDirection: "column", gap: 10 }}>
                <div style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 700 }}>Promote · {proposal.description}</div>
                <div style={{ fontSize: 12, color: "var(--ink-mute)", lineHeight: 1.5 }}>{proposal.note}</div>
                {proposal.changes.map((c, i) => (
                  <div
                    key={c.id || String(i)}
                    style={{
                      padding: "10px 0",
                      borderTop: i > 0 ? "1px solid var(--hairline)" : "none",
                      display: "flex",
                      gap: 10,
                      alignItems: "flex-start",
                    }}
                  >
                    {c.applyable ? (
                      <input
                        type="checkbox"
                        aria-label={`Approve: ${c.title}`}
                        checked={approved.has(c.id)}
                        onChange={() => setApproved((prev) => { const n = new Set(prev); if (n.has(c.id)) n.delete(c.id); else n.add(c.id); return n; })}
                        style={{ marginTop: 3, flexShrink: 0, width: 16, height: 16 }}
                      />
                    ) : null}
                    <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: 4 }}>
                      <div style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
                        <span style={{ fontSize: 13, fontWeight: 650 }}>{c.title}</span>
                        <span className={`chip ${c.applyable ? "accent" : ""}`} style={{ fontSize: 11 }}>{c.applyable ? "Applyable" : "Recommendation"}</span>
                      </div>
                      <div style={{ fontSize: 12, color: "var(--ink-mute)", lineHeight: 1.45 }}>{c.detail}</div>
                      {c.currentCents != null && c.proposedCents != null ? (
                        <div className="num" style={{ fontSize: 12, color: "var(--ink-2)" }}>
                          <span className="money">{fmt(c.currentCents as number)}</span>
                          <span style={{ color: "var(--ink-faint)", margin: "0 6px" }}>→</span>
                          <span className="money">{fmt(c.proposedCents as number)}</span>
                        </div>
                      ) : null}
                    </div>
                  </div>
                ))}
                {proposal.changes.filter((c) => c.applyable).length > 0 ? (
                  <div style={{ display: "flex", gap: 8 }}>
                    <button
                      type="button"
                      className="btn primary"
                      style={{ flex: 1, minHeight: 44 }}
                      disabled={apply.isPending || approved.size === 0}
                      onClick={async () => {
                        try {
                          const res = await apply.mutateAsync({ id: proposal.scenarioId, approvedChangeIds: [...approved] });
                          toast.success("Applied to your plan", { description: res.note });
                          setProposal(null);
                        } catch (e) {
                          toast.error("Could not apply scenario", { description: userErrorMessage(e, "Try again.") });
                        }
                      }}
                    >
                      Apply {approved.size} to plan
                    </button>
                    <button type="button" className="btn" style={{ minHeight: 44 }} onClick={() => setProposal(null)}>
                      Close
                    </button>
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
        ) : null}
        {selected ? (
          <StickyActionBar ariaLabel="Scenario actions">
            <button
              type="button"
              className="btn"
              style={{ minHeight: 44 }}
              disabled={promote.isPending || !selected.recomputable}
              onClick={async () => {
                try {
                  const res = await promote.mutateAsync(selected.id);
                  setProposal(res);
                  setApproved(new Set(res.changes.filter((c) => c.applyable).map((c) => c.id)));
                } catch (e) {
                  toast.error("Could not promote scenario", { description: userErrorMessage(e, "Re-run and save it first.") });
                }
              }}
            >
              {promote.isPending ? "…" : "Promote"}
            </button>
            <button
              type="button"
              className="btn"
              style={{ minHeight: 44 }}
              disabled={dup.isPending}
              onClick={async () => {
                try {
                  await dup.mutateAsync(selected.id);
                  toast.success("Scenario duplicated");
                } catch {
                  toast.error("Could not duplicate scenario");
                }
              }}
            >
              Duplicate
            </button>
            <button
              type="button"
              className="btn"
              style={{ minHeight: 44 }}
              disabled={archive.isPending}
              onClick={async () => {
                try {
                  await archive.mutateAsync({ id: selected.id, archived: true });
                  toast("Scenario archived");
                  closeDetail();
                } catch {
                  toast.error("Could not archive scenario");
                }
              }}
            >
              Archive
            </button>
            <button
              type="button"
              className="btn"
              aria-label="Delete scenario"
              style={{ minHeight: 44, color: "var(--negative)", borderColor: "color-mix(in oklab, var(--negative) 30%, var(--line))" }}
              disabled={del.isPending}
              onClick={async () => {
                try {
                  await del.mutateAsync(selected.id);
                  toast("Scenario deleted");
                  closeDetail();
                } catch {
                  toast.error("Could not delete scenario");
                }
              }}
            >
              <I.Trash width={14} height={14} />
            </button>
          </StickyActionBar>
        ) : null}
      </BottomSheet>
    </div>
  );
}
