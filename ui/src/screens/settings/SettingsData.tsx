import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { useFinancialMetrics, useSetFinancialAssumptions, useFinancialPhilosophy, useSetFinancialPhilosophy } from "../../api/hooks/metrics";
import { useAgentMemory, useForgetAgentMemory, useUpsertAgentMemory } from "../../api/hooks/agentMemory";
import { useDataHealth, useCreateBackup, useStageRestore, useCancelRestore } from "../../api/hooks/dataHealth";
import { useExportJson, useExportCsv, useAutoCategorizeEnabled, useSetAutoCategorizeEnabled } from "../../api/hooks/settings";
import { useCompletionProvider, useSetCompletionProvider, useSaveProviderApiKey, useTestCompletionProvider, useTriggerCategorize, useListProviderModels } from "../../api/hooks/agent";
import { useTweaks } from "../../state/tweaks";

import DeleteAllDataDialog from "../../components/DeleteAllDataDialog";
import { Toggle as Tog } from "../../components/Toggle";
import { Section } from "./Section";
import type { CompletionProviderConfig } from "../../api/openapiClient";
import { userErrorMessage } from "../../utils/runtime";

type ProviderKind = "ollama" | "openai_compat" | "anthropic" | null;

const OPENAI_COMPAT_PRESETS = [
  { label: "OpenAI", preset: "openai", base_url: "https://api.openai.com/v1" },
  { label: "OpenRouter", preset: "openrouter", base_url: "https://openrouter.ai/api/v1" },
  { label: "Google", preset: "google", base_url: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { label: "Custom", preset: "custom", base_url: "" },
] as const;
type CompatPreset = (typeof OPENAI_COMPAT_PRESETS)[number];

function providerDisplayName(cfg: CompletionProviderConfig | undefined) {
  if (!cfg || cfg.kind === "unconfigured") return "Not configured";
  if (cfg.kind === "ollama") return `Configured — Ollama (${cfg.model})`;
  if (cfg.kind === "anthropic") return `Configured — Anthropic (${cfg.model})`;
  return `Configured — ${cfg.preset} (${cfg.model})`;
}

const DEBT_STRATEGIES = [
  { value: "avalanche", label: "Highest interest first", detail: "Avalanche — pays the least interest overall." },
  { value: "snowball", label: "Smallest balance first", detail: "Snowball — early wins keep the momentum up (Ramsey)." },
] as const;

const RISK_TOLERANCES = [
  { value: "cautious", label: "Debt-averse", detail: "Clear debt even when the math slightly favours investing." },
  { value: "balanced", label: "Balanced", detail: "The default: weigh clearing debt and investing evenly." },
  { value: "aggressive", label: "Optimise the math", detail: "Take the mathematically optimal answer regardless of how it feels." },
] as const;

export function PhilosophySection() {
  const { data: philosophy } = useFinancialPhilosophy();
  const save = useSetFinancialPhilosophy();
  const [debtStrategy, setDebtStrategy] = useState("avalanche");
  const [riskTolerance, setRiskTolerance] = useState("balanced");
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    if (philosophy && !dirty) {
      setDebtStrategy(philosophy.debtStrategy);
      setRiskTolerance(philosophy.riskTolerance);
    }
  }, [philosophy, dirty]);

  const onSave = async () => {
    try {
      await save.mutateAsync({
        debtStrategy,
        riskTolerance,
        highInterestAprPct: philosophy?.highInterestAprPct ?? 8,
      });
      setDirty(false);
      toast.success("Advice preferences saved");
    } catch (error) {
      toast.error("Could not save preferences", { description: userErrorMessage(error) });
    }
  };

  const choice = (
    name: string,
    label: string,
    desc: string,
    options: ReadonlyArray<{ value: string; label: string; detail: string }>,
    value: string,
    setter: (v: string) => void,
  ) => (
    <div className="s-row">
      <div>
        <div className="label">{label}</div>
        <div className="desc">{desc}</div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {options.map((option) => (
          <label key={option.value} className="row row-sm" style={{ alignItems: "flex-start", gap: 8 }}>
            <input
              type="radio"
              name={name}
              value={option.value}
              checked={value === option.value}
              onChange={() => {
                setter(option.value);
                setDirty(true);
              }}
            />
            <span>
              <span style={{ fontWeight: 600 }}>{option.label}</span>
              <span className="desc" style={{ display: "block" }}>
                {option.detail}
              </span>
            </span>
          </label>
        ))}
      </div>
      <div />
    </div>
  );

  return (
    <Section
      id="philosophy"
      title="How you want advice"
      description="The books this app draws on disagree with each other, and both sides are defensible. Tell FinSight which you follow and the Copilot — and the debt engine behind it — will argue your way."
    >
      {choice("debt-strategy", "Debt payoff order", "Which debt to attack first when you have spare money.", DEBT_STRATEGIES, debtStrategy, setDebtStrategy)}
      {choice(
        "risk-tolerance",
        "Debt versus investing",
        philosophy ? `Currently treating debt at or above ${philosophy.highInterestAprPct}% APR as urgent.` : "Where the line sits between paying debt down and investing instead.",
        RISK_TOLERANCES,
        riskTolerance,
        setRiskTolerance,
      )}
      <div className="s-row">
        <div />
        <div style={{ textAlign: "right" }}>
          <button className="btn primary sm" type="button" disabled={save.isPending || !dirty} onClick={() => void onSave()}>
            {save.isPending ? "Applying…" : "Apply preferences"}
          </button>
        </div>
        <div />
      </div>
    </Section>
  );
}

function FinancialTargetsSection() {
  const { data: metrics } = useFinancialMetrics();
  const save = useSetFinancialAssumptions();
  const [savingsRate, setSavingsRate] = useState("");
  const [efMonths, setEfMonths] = useState("");
  const [returnPct, setReturnPct] = useState("");
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    if (metrics && !dirty) {
      setSavingsRate(String(metrics.targetSavingsRatePct));
      setEfMonths(String(metrics.emergencyFundTargetMonths));
      setReturnPct(String(metrics.expectedAnnualReturnPct));
    }
  }, [metrics, dirty]);

  const onSave = async () => {
    try {
      await save.mutateAsync({
        targetSavingsRatePct: Math.round(Number(savingsRate) || 0),
        emergencyFundTargetMonths: Number(efMonths) || 0,
        expectedAnnualReturnPct: Number(returnPct) || 0,
      });
      setDirty(false);
      toast.success("Financial targets saved");
    } catch (error) {
      toast.error("Could not save targets", { description: userErrorMessage(error) });
    }
  };

  const field = (label: string, desc: string, value: string, setter: (v: string) => void, suffix: string, step: string) => (
    <div className="s-row">
      <div>
        <div className="label">{label}</div>
        <div className="desc">{desc}</div>
      </div>
      <div className="row row-sm" style={{ alignItems: "center", justifyContent: "flex-end" }}>
        <input
          className="control"
          type="number"
          min="0"
          step={step}
          value={value}
          onChange={(e) => {
            setter(e.target.value);
            setDirty(true);
          }}
          aria-label={label}
          style={{ maxWidth: 100 }}
        />
        <span className="muted">{suffix}</span>
      </div>
      <div />
    </div>
  );

  return (
    <Section
      id="targets"
      title="Financial targets"
      description="The assumptions behind your scorecard, journey, and projections. Change them here and every screen — and the Copilot — follows the same numbers."
    >
      {field("Target savings rate", "Pay-yourself-first floor used by the health score and savings nudges.", savingsRate, setSavingsRate, "%", "1")}
      {field("Emergency fund target", "Months of expenses a full emergency fund should cover (Ramsey: 3–6).", efMonths, setEfMonths, "months", "0.5")}
      {field("Expected annual return", "Long-run growth the compound projector assumes when a goal has no linked account APY.", returnPct, setReturnPct, "% / yr", "0.5")}
      <div className="s-row">
        <div />
        <div style={{ textAlign: "right" }}>
          <button className="btn primary sm" type="button" disabled={save.isPending || !dirty} onClick={() => void onSave()}>
            {save.isPending ? "Applying…" : "Apply targets"}
          </button>
        </div>
        <div />
      </div>
    </Section>
  );
}

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function fmtWhen(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return isNaN(d.getTime()) ? "—" : d.toLocaleString();
}

function DataBackupsSection() {
  const { data: health, isLoading } = useDataHealth();
  const backup = useCreateBackup();
  const stageRestore = useStageRestore();
  const cancelRestore = useCancelRestore();
  const integrityOk = (health?.integrityStatus ?? "").trim() === "ok";

  return (
    <Section id="backups" title="Data & backups" description="Your data is encrypted on this device. FinSight snapshots it before every update; you can also back up on demand and restore a snapshot.">
      {isLoading ? (
        <div className="muted">Checking data health…</div>
      ) : (
        <>
          <div className="s-row">
            <div>
              <div className="label">Database integrity</div>
              <div className="desc">Last checked {fmtWhen(health?.integrityCheckedAt)}.</div>
            </div>
            <div className="row row-sm" style={{ justifyContent: "flex-end", alignItems: "center" }}>
              <span className={`chip ${integrityOk ? "positive" : "warning"}`}>{integrityOk ? "Healthy" : health?.integrityStatus || "Unknown"}</span>
            </div>
            <div />
          </div>

          {health && health.startupWarnings.length > 0 && (
            <div className="card" style={{ borderColor: "var(--negative)", marginBottom: 12 }}>
              <div className="label" style={{ color: "var(--negative)" }}>Some background updates didn&apos;t finish</div>
              <ul className="muted" style={{ margin: "8px 0 0 16px", fontSize: 12.5 }}>
                {health.startupWarnings.map((w, i) => <li key={i}>{w}</li>)}
              </ul>
              <div className="muted" style={{ marginTop: 6, fontSize: 12 }}>Some numbers may be momentarily stale. Restarting usually clears this.</div>
            </div>
          )}

          {health && health.startupSummary && (
            <div className="s-row">
              <div>
                <div className="label">Launch refresh</div>
                <div className="desc">{health.startupSummary}</div>
              </div>
              <div />
              <div />
            </div>
          )}

          {health?.pendingRestore && (
            <div className="card accent" style={{ marginBottom: 12 }}>
              <div className="label">A restore is staged</div>
              <div className="muted" style={{ marginTop: 4, fontSize: 12.5 }}>It will be applied the next time you restart FinSight.</div>
              <button
                className="btn ghost sm"
                type="button"
                style={{ marginTop: 10 }}
                disabled={cancelRestore.isPending}
                onClick={async () => {
                  try {
                    await cancelRestore.mutateAsync();
                    toast.success("Restore cancelled");
                  } catch (e) {
                    toast.error("Could not cancel", { description: userErrorMessage(e) });
                  }
                }}
              >
                Cancel staged restore
              </button>
            </div>
          )}

          <div className="s-row">
            <div>
              <div className="label">Storage</div>
              <div className="desc">Database {fmtBytes(health?.dbBytes ?? 0)} · write-ahead log {fmtBytes(health?.walBytes ?? 0)}.</div>
            </div>
            <div className="row row-sm" style={{ justifyContent: "flex-end" }}>
              <button
                className="btn primary sm"
                type="button"
                disabled={backup.isPending}
                onClick={async () => {
                  try {
                    const b = await backup.mutateAsync();
                    toast.success("Backup created", { description: b.name });
                  } catch (e) {
                    toast.error("Backup failed", { description: userErrorMessage(e) });
                  }
                }}
              >
                {backup.isPending ? "Backing up…" : "Back up now"}
              </button>
            </div>
            <div />
          </div>

          <div style={{ marginTop: 14 }}>
            <div className="label" style={{ marginBottom: 8 }}>Snapshots</div>
            {!health || health.backups.length === 0 ? (
              <div className="muted" style={{ fontSize: 13 }}>No backups yet. One is created automatically before each app update.</div>
            ) : (
              <div className="tbl" role="list" aria-label="Available backups">
                {health.backups.map((b) => (
                  <div key={b.path} className="row" role="listitem" style={{ alignItems: "center", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid var(--line)" }}>
                    <div style={{ minWidth: 0 }}>
                      <div className="mono" style={{ fontSize: 12.5, overflow: "hidden", textOverflow: "ellipsis" }}>{b.name.replace(/^data\.backup-/, "").replace(/\.sqlcipher$/, "")}</div>
                      <div className="muted" style={{ fontSize: 11.5 }}>{fmtWhen(b.createdAt)} · {fmtBytes(b.bytes)}</div>
                    </div>
                    <button
                      className="btn ghost sm"
                      type="button"
                      disabled={stageRestore.isPending}
                      onClick={async () => {
                        try {
                          await stageRestore.mutateAsync(b.path);
                          toast.success("Restore staged", { description: "Restart FinSight to apply." });
                        } catch (e) {
                          toast.error("Could not stage restore", { description: userErrorMessage(e) });
                        }
                      }}
                    >
                      Restore…
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </>
      )}
    </Section>
  );
}

function AgentMemoryPanel() {
  const { data: memory = [] } = useAgentMemory();
  const forgetMemory = useForgetAgentMemory();
  const [pendingForget, setPendingForget] = useState<Set<string>>(new Set());
  const forgetTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  useEffect(() => {
    const timers = forgetTimers.current;
    return () => {
      timers.forEach((t) => clearTimeout(t));
      timers.clear();
    };
  }, []);

  const handleForget = (m: { id: string; description: string }) => {
    setPendingForget((s) => new Set([...s, m.id]));
    const timer = setTimeout(async () => {
      forgetTimers.current.delete(m.id);
      try {
        await forgetMemory.mutateAsync(m.id);
      } catch {
        toast.error("Could not forget that memory");
      }
      setPendingForget((s) => {
        const n = new Set(s);
        n.delete(m.id);
        return n;
      });
    }, 5000);
    forgetTimers.current.set(m.id, timer);
    toast("Memory forgotten", {
      description: m.description.slice(0, 60),
      action: {
        label: "Undo",
        onClick: () => {
          const t = forgetTimers.current.get(m.id);
          if (t) {
            clearTimeout(t);
            forgetTimers.current.delete(m.id);
          }
          setPendingForget((s) => {
            const n = new Set(s);
            n.delete(m.id);
            return n;
          });
        },
      },
    });
  };

  const visibleMemory = memory.filter((m) => !pendingForget.has(m.id));

  return (
    <div className="s-row" style={{ alignItems: "flex-start" }}>
      <div>
        <div className="label">What the agent has learned</div>
        <div className="desc">Corrections and preferences the agent remembers about your finances. Forget any that are wrong or stale.</div>
      </div>
      <div style={{ gridColumn: "2 / -1" }}>
        {visibleMemory.length === 0 ? (
          <div className="muted" style={{ fontSize: 13 }}>Nothing remembered yet — the agent learns as you correct categories and confirm patterns.</div>
        ) : (
          <ul className="stack" style={{ margin: 0, padding: 0, listStyle: "none", width: "100%" }}>
            {visibleMemory.map((m) => (
              <li key={m.id} className="row-md" style={{ padding: "8px 0", borderBottom: "1px solid var(--hairline)", alignItems: "center" }}>
                <div className="grow" style={{ fontSize: 13.5, minWidth: 0 }}><span className="chip" style={{ marginRight: 8, fontSize: 11 }}>{m.kind}</span>{m.description}</div>
                <button className="btn ghost sm" type="button" onClick={() => handleForget(m)} aria-label={`Forget: ${m.description}`}>
                  Forget
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function PreferenceMemoryPanel() {
  const { data: memory = [] } = useAgentMemory();
  const upsert = useUpsertAgentMemory();
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const preferences = memory.filter((m) => m.kind === "preference" || m.kind === "philosophy" || m.kind === "risk_tolerance");
  const handleSave = async () => {
    if (!key.trim() || !value.trim()) {
      toast.error("Key and value required");
      return;
    }
    try {
      await upsert.mutateAsync({ kind: "preference", key: key.trim(), description: value.trim() });
      toast.success("Preference saved", { description: `${key.trim()}: ${value.trim().slice(0, 40)}` });
      setKey("");
      setValue("");
    } catch (e) {
      toast.error(userErrorMessage(e));
    }
  };
  return (
    <div className="s-row" style={{ alignItems: "flex-start" }}>
      <div>
        <div className="label">Your preferences</div>
        <div className="desc">Tell the agent how you want advice — risk tolerance, philosophy, or any preference. Stored as controlled memory and shown to the model.</div>
      </div>
      <div style={{ gridColumn: "2 / -1", display: "flex", flexDirection: "column", gap: 12, width: "100%" }}>
        {preferences.length > 0 ? (
          <ul className="stack" style={{ margin: 0, padding: 0, listStyle: "none", width: "100%" }}>
            {preferences.map((m) => (
              <li key={m.id} className="row-md" style={{ padding: "8px 0", borderBottom: "1px solid var(--hairline)", alignItems: "center" }}>
                <div className="grow" style={{ fontSize: 13.5, minWidth: 0 }}><span className="chip" style={{ marginRight: 8, fontSize: 11 }}>{m.merchantKey ?? m.kind}</span>{m.description}</div>
                <span className="muted" style={{ fontSize: 11 }}>{m.kind}</span>
              </li>
            ))}
          </ul>
        ) : (
          <div className="muted" style={{ fontSize: 13 }}>No preferences yet — add one below.</div>
        )}
        <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
          <input className="control" placeholder="Key (e.g. risk_tolerance)" value={key} onChange={(e) => setKey(e.target.value)} style={{ flex: "1 1 160px", minWidth: 0 }} aria-label="Preference key" />
          <input className="control" placeholder="Value (e.g. cautious)" value={value} onChange={(e) => setValue(e.target.value)} style={{ flex: "2 1 220px", minWidth: 0 }} aria-label="Preference value" />
          <button className="btn primary sm" type="button" onClick={handleSave} disabled={upsert.isPending}>Save</button>
        </div>
      </div>
    </div>
  );
}

function AgentSection() {
  const { data: autoCategorizeEnabled = true } = useAutoCategorizeEnabled();
  const setAutoCategorizeMutation = useSetAutoCategorizeEnabled();
  return (
    <Section id="agent" title="Agent" description="Control what the agent does automatically, and what it remembers.">
      <div className="s-row">
        <div>
          <div className="label">Auto-categorize new transactions</div>
          <div className="desc">Automatically categorize transactions after each import or sync, using your configured AI provider.</div>
        </div>
        <div className="muted">{autoCategorizeEnabled ? "Currently on" : "Currently off"}</div>
        <Tog checked={autoCategorizeEnabled} onChange={(value) => setAutoCategorizeMutation.mutate(value)} ariaLabel="Auto-categorize new transactions" />
      </div>
      <AgentMemoryPanel />
      <PreferenceMemoryPanel />
      <div className="card tight" style={{ marginTop: 12 }}>
        <div className="row row-sm" style={{ alignItems: "flex-start", gap: 8 }}>
          <span aria-hidden style={{ fontSize: 15 }}>🔒</span>
          <div className="muted" style={{ fontSize: 12.5, lineHeight: 1.5 }}>
            <strong style={{ color: "var(--ink)" }}>What leaves FinSight.</strong> When auto-categorize is on and you use a <em>cloud</em> AI provider (OpenAI-compatible or Anthropic), the merchant description and amount of each <em>uncategorized</em> transaction are
            sent to that provider to pick a category. Balances, account numbers, and totals are never sent. Transaction reference numbers, and the names of people in e-transfers, are redacted before sending. Choose{" "}
            <strong>Ollama</strong> inside your self-hosted setup to keep processing under your control, or turn auto-categorize off to categorize manually.
          </div>
        </div>
      </div>
    </Section>
  );
}

function ProviderSection() {
  const { data: currentProvider } = useCompletionProvider();
  const setProvider = useSetCompletionProvider();
  const saveKey = useSaveProviderApiKey();
  const testProvider = useTestCompletionProvider();
  const triggerCategorize = useTriggerCategorize();
  const [providerPanelOpen, setProviderPanelOpen] = useState(false);
  const [selectedKind, setSelectedKind] = useState<ProviderKind>(null);
  const [selectedPreset, setSelectedPreset] = useState<CompatPreset>(OPENAI_COMPAT_PRESETS[0]);
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [ollamaModel, setOllamaModel] = useState("");
  const [compatModel, setCompatModel] = useState("");
  const [anthropicModel, setAnthropicModel] = useState("claude-3-5-haiku-latest");
  const [apiKey, setApiKey] = useState("");
  const [testResult, setTestResult] = useState<{ ok: boolean; latency_ms: number; error: string | null } | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const effectiveModelsConfig = (() => {
    if (selectedKind !== "ollama") return null;
    return { kind: "ollama", base_url: ollamaUrl, model: ollamaModel } as CompletionProviderConfig;
  })();
  const { data: ollamaModels = [] } = useListProviderModels(effectiveModelsConfig);

  useEffect(() => {
    if (!providerPanelOpen || !currentProvider) return;
    if (currentProvider.kind === "ollama") {
      setSelectedKind("ollama");
      setOllamaUrl(currentProvider.base_url);
      setOllamaModel(currentProvider.model);
    } else if (currentProvider.kind === "openai_compat") {
      setSelectedKind("openai_compat");
      setSelectedPreset(OPENAI_COMPAT_PRESETS.find((preset) => preset.preset === currentProvider.preset) ?? OPENAI_COMPAT_PRESETS[0]);
      setCompatModel(currentProvider.model);
    } else if (currentProvider.kind === "anthropic") {
      setSelectedKind("anthropic");
      setAnthropicModel(currentProvider.model);
    } else {
      setSelectedKind(null);
    }
  }, [currentProvider, providerPanelOpen]);

  const buildConfig = (): CompletionProviderConfig | null => {
    if (selectedKind === "ollama") return { kind: "ollama", base_url: ollamaUrl, model: ollamaModel };
    if (selectedKind === "openai_compat") return { kind: "openai_compat", preset: selectedPreset.preset, base_url: selectedPreset.base_url, model: compatModel };
    if (selectedKind === "anthropic") return { kind: "anthropic", model: anthropicModel };
    return null;
  };

  const handleTestConnection = async () => {
    const config = buildConfig();
    if (!config) return;
    try {
      const result = await testProvider.mutateAsync({ config, apiKey: apiKey.trim() || undefined });
      setTestResult({ ok: result.ok, latency_ms: result.latency_ms, error: result.error ?? null });
    } catch (error) {
      setTestResult({ ok: false, latency_ms: 0, error: userErrorMessage(error, "Connection failed.") });
    }
  };

  const handleSave = async () => {
    const config = buildConfig();
    if (!config) return;
    setSaveError(null);
    try {
      const trimmedKey = apiKey.trim();
      if (trimmedKey && selectedKind && selectedKind !== "ollama") {
        const providerId = selectedKind === "anthropic" ? "anthropic" : selectedPreset.preset;
        await saveKey.mutateAsync({ providerId, key: trimmedKey });
      }
      await setProvider.mutateAsync(config);
      setProviderPanelOpen(false);
    } catch (error) {
      setSaveError(userErrorMessage(error, "Could not save provider settings."));
    }
  };

  return (
    <Section id="provider" title="AI Provider" description="Choose where categorization and forecasting run.">
      {!providerPanelOpen ? (
        <div className="card tight">
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 16 }}>
            <div className="muted">{providerDisplayName(currentProvider)}</div>
            <button className="btn sm" type="button" onClick={() => setProviderPanelOpen(true)}>
              {currentProvider && currentProvider.kind !== "unconfigured" ? "Edit" : "Configure"}
            </button>
          </div>
        </div>
      ) : (
        <div className="card">
          <div className="toolbar" style={{ marginBottom: 18 }}>
            <button className={selectedKind === "ollama" ? "on" : ""} type="button" onClick={() => { setSelectedKind("ollama"); setApiKey(""); }}>
              Ollama
            </button>
            <button className={selectedKind === "openai_compat" ? "on" : ""} type="button" onClick={() => { setSelectedKind("openai_compat"); setApiKey(""); }}>
              Cloud
            </button>
            <button className={selectedKind === "anthropic" ? "on" : ""} type="button" onClick={() => { setSelectedKind("anthropic"); setApiKey(""); }}>
              Anthropic
            </button>
          </div>

          {selectedKind === "ollama" && (
            <div className="stack stack-md">
              <label className="stack stack-xs">
                <span className="muted">Base URL</span>
                <input className="control" value={ollamaUrl} onChange={(e) => setOllamaUrl(e.target.value)} />
              </label>
              <label className="stack stack-xs">
                <span className="muted">Model</span>
                <select className="control" value={ollamaModel} onChange={(e) => setOllamaModel(e.target.value)}>
                  {ollamaModels.map((model) => (
                    <option key={model} value={model}>
                      {model}
                    </option>
                  ))}
                  {ollamaModels.length === 0 && <option value="">Pick a model</option>}
                </select>
              </label>
            </div>
          )}
          {selectedKind === "openai_compat" && (
            <div className="stack stack-md">
              <div className="row row-sm wrap">
                {OPENAI_COMPAT_PRESETS.map((preset) => (
                  <button
                    key={preset.preset}
                    className={`btn ${selectedPreset.preset === preset.preset ? "primary" : "outline"} sm`}
                    type="button"
                    onClick={() => setSelectedPreset(preset)}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>
              <label className="stack stack-xs">
                <span className="muted">Model</span>
                <input className="control" value={compatModel} onChange={(e) => setCompatModel(e.target.value)} placeholder="e.g. gpt-4o-mini" />
              </label>
              <label className="stack stack-xs">
                <span className="muted">API key</span>
                <input className="control" type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-…" />
              </label>
            </div>
          )}
          {selectedKind === "anthropic" && (
            <div className="stack stack-md">
              <label className="stack stack-xs">
                <span className="muted">Model</span>
                <input className="control" value={anthropicModel} onChange={(e) => setAnthropicModel(e.target.value)} />
              </label>
              <label className="stack stack-xs">
                <span className="muted">API key</span>
                <input className="control" type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-ant-…" />
              </label>
            </div>
          )}

          {testResult && <div className="muted" style={{ marginTop: 14 }}>{testResult.ok ? `Connected — ${testResult.latency_ms}ms` : testResult.error}</div>}
          {saveError && <div className="muted" style={{ marginTop: 14 }}>{saveError}</div>}

          <div className="row row-sm wrap" style={{ marginTop: 18 }}>
            <button className="btn sm" type="button" onClick={() => void handleTestConnection()}>
              Test connection
            </button>
            <button className="btn primary sm" type="button" onClick={() => void handleSave()}>
              Save
            </button>
            <button className="btn ghost sm" type="button" onClick={() => { setProviderPanelOpen(false); setTestResult(null); }}>
              Cancel
            </button>
            <button className="btn outline sm" type="button" onClick={() => triggerCategorize.mutate()}>
              Re-categorize all
            </button>
          </div>
        </div>
      )}
    </Section>
  );
}

function PrivacySection() {
  const exportJson = useExportJson();
  const exportCsv = useExportCsv();
  const { privacy, setPrivacy } = useTweaks();
  const [deleteAllOpen, setDeleteAllOpen] = useState(false);
  return (
    <>
      <Section id="privacy" title="Privacy & data" description="Keep control of your data and what appears on-screen.">
        <div className="s-row">
          <div>
            <div className="label">Privacy mode</div>
            <div className="desc">Blur displayed amounts when you are sharing your screen or want extra discretion.</div>
          </div>
          <div className="muted">Shortcut: ⌘.</div>
          <Tog checked={privacy} onChange={setPrivacy} ariaLabel="Privacy mode" />
        </div>
        <div className="s-row">
          <div>
            <div className="label">Export data</div>
            <div className="desc">Download the full dataset as JSON or CSV whenever you want a local backup.</div>
          </div>
          <div className="row row-sm wrap">
            <button
              className="btn sm"
              type="button"
              onClick={async () => {
                try {
                  await exportJson.mutateAsync();
                  toast.success("File saved");
                } catch (error) {
                  toast.error("Export failed", { description: userErrorMessage(error, "Check your FinSight server connection and try exporting again.") });
                }
              }}
            >
              Export as JSON
            </button>
            <button
              className="btn sm"
              type="button"
              onClick={async () => {
                try {
                  await exportCsv.mutateAsync();
                  toast.success("File saved");
                } catch (error) {
                  toast.error("Export failed", { description: userErrorMessage(error, "Check your FinSight server connection and try exporting again.") });
                }
              }}
            >
              Export as CSV
            </button>
          </div>
          <div />
        </div>
        <div className="s-row">
          <div>
            <div className="label">Delete all data</div>
            <div className="desc">Permanently remove every account, transaction, balance, budget, goal, insight, and agent memory from this FinSight profile. Your AI provider settings and API keys are kept. This cannot be undone.</div>
          </div>
          <div>
            <button className="btn danger sm" type="button" onClick={() => setDeleteAllOpen(true)}>
              Delete all data
            </button>
          </div>
          <div />
        </div>
      </Section>
      <DeleteAllDataDialog open={deleteAllOpen} onClose={() => setDeleteAllOpen(false)} />
    </>
  );
}

export default function SettingsData() {
  return (
    <>
      <FinancialTargetsSection />
      <PhilosophySection />
      <DataBackupsSection />
      <PrivacySection />
      <AgentSection />
      <ProviderSection />
    </>
  );
}
