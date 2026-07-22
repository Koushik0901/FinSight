import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { fetchAuthStatus, isServerMode, logout } from "../api/auth";
import { useResetOnboarding, useOnboardingState } from "../api/hooks/onboarding";
import {
  useCompletionProvider,
  useSetCompletionProvider,
  useSaveProviderApiKey,
  useTestCompletionProvider,
  useTriggerCategorize,
  useListProviderModels,
} from "../api/hooks/agent";
import { useDefaultCurrency, useSetCurrency, useExportJson, useExportCsv, useAutoCategorizeEnabled, useSetAutoCategorizeEnabled } from "../api/hooks/settings";
import {
  useFinancialMetrics,
  useSetFinancialAssumptions,
  useFinancialPhilosophy,
  useSetFinancialPhilosophy,
} from "../api/hooks/metrics";
import { useAgentMemory, useForgetAgentMemory } from "../api/hooks/agentMemory";
import { useDataHealth, useCreateBackup, useStageRestore, useCancelRestore } from "../api/hooks/dataHealth";
import {
  useSimpleFinStatus,
  useDisconnectSimpleFin,
  usePurgeSimpleFinData,
  useSimpleFinConnections,
  useDeleteSimpleFinConnection,
  useSimpleFinSyncSettings,
  useSetSimpleFinSyncSettings,
} from "../api/hooks/simplefin";
import SimpleFinDialog from "./onboarding/SimpleFinDialog";
import DeleteAllDataDialog from "../components/DeleteAllDataDialog";
import PushNotificationSettings from "../components/PushNotificationSettings";
import NotificationPolicySettings from "../components/NotificationPolicySettings";
import { Toggle as Tog } from "../components/Toggle";
import { useTweaks, ACCENTS, type AccentId } from "../state/tweaks";
import type { CompletionProviderConfig } from "../api/client";
import { userErrorMessage } from "../utils/runtime";

type ProviderKind = "ollama" | "openai_compat" | "anthropic" | null;

const OPENAI_COMPAT_PRESETS = [
  { label: "OpenAI", preset: "openai", base_url: "https://api.openai.com/v1" },
  { label: "OpenRouter", preset: "openrouter", base_url: "https://openrouter.ai/api/v1" },
  { label: "Google", preset: "google", base_url: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { label: "Custom", preset: "custom", base_url: "" },
] as const;
type CompatPreset = (typeof OPENAI_COMPAT_PRESETS)[number];

const CURRENCIES = ["USD", "EUR", "GBP", "CAD", "AUD", "JPY"];
const SECTIONS = [
  ["profile", "Profile"],
  ["targets", "Financial targets"],
  ["philosophy", "How you want advice"],
  ["privacy", "Privacy & data"],
  ["backups", "Data & backups"],
  ["agent", "Agent"],
  ["provider", "AI Provider"],
  ["appearance", "Appearance"],
  ["connections", "Connections"],
  ["notifications", "Notifications"],
  ["keyboard", "Keyboard"],
  ["about", "About"],
] as const;
const SECTION_IDS = SECTIONS.map(([id]) => id);
// Server-mode-only nav entry (Sign out) — appended when isServerMode() so the
// desktop app's nav/section list is byte-identical to before this feature.
const SERVER_ACCOUNT_SECTION = ["account", "Account"] as const;

function providerDisplayName(cfg: CompletionProviderConfig | undefined) {
  if (!cfg || cfg.kind === "unconfigured") return "Not configured";
  if (cfg.kind === "ollama") return `Configured — Ollama (${cfg.model})`;
  if (cfg.kind === "anthropic") return `Configured — Anthropic (${cfg.model})`;
  return `Configured — ${cfg.preset} (${cfg.model})`;
}

const DEBT_STRATEGIES = [
  {
    value: "avalanche",
    label: "Highest interest first",
    detail: "Avalanche — pays the least interest overall.",
  },
  {
    value: "snowball",
    label: "Smallest balance first",
    detail: "Snowball — early wins keep the momentum up (Ramsey).",
  },
] as const;

const RISK_TOLERANCES = [
  {
    value: "cautious",
    label: "Debt-averse",
    detail: "Clear debt even when the math slightly favours investing.",
  },
  {
    value: "balanced",
    label: "Balanced",
    detail: "The default: weigh clearing debt and investing evenly.",
  },
  {
    value: "aggressive",
    label: "Optimise the math",
    detail: "Take the mathematically optimal answer regardless of how it feels.",
  },
] as const;

/** Which school of personal-finance advice the user subscribes to.
 *
 *  These are not cosmetic: they set the default debt-payoff ordering used by
 *  the ranking engine and the APR above which debt is treated as urgent, and
 *  they are stated in the Copilot's prompt. Defaults reproduce the behaviour
 *  the app had before the setting existed. */
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
        // Derived server-side from riskTolerance; sent only to satisfy the
        // shared type.
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
              <span className="desc" style={{ display: "block" }}>{option.detail}</span>
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
      {choice(
        "debt-strategy",
        "Debt payoff order",
        "Which debt to attack first when you have spare money.",
        DEBT_STRATEGIES,
        debtStrategy,
        setDebtStrategy,
      )}
      {choice(
        "risk-tolerance",
        "Debt versus investing",
        philosophy
          ? `Currently treating debt at or above ${philosophy.highInterestAprPct}% APR as urgent.`
          : "Where the line sits between paying debt down and investing instead.",
        RISK_TOLERANCES,
        riskTolerance,
        setRiskTolerance,
      )}
      <div className="s-row">
        <div />
        <div style={{ textAlign: "right" }}>
          <button
            className="btn primary sm"
            type="button"
            disabled={save.isPending || !dirty}
            onClick={() => void onSave()}
          >
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

  // Seed the inputs from the stored assumptions once loaded; don't clobber edits.
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
      <div><div className="label">{label}</div><div className="desc">{desc}</div></div>
      <div className="row row-sm" style={{ alignItems: "center", justifyContent: "flex-end" }}>
        <input className="control" type="number" min="0" step={step} value={value} onChange={(e) => { setter(e.target.value); setDirty(true); }} aria-label={label} style={{ maxWidth: 100 }} />
        <span className="muted">{suffix}</span>
      </div>
      <div />
    </div>
  );

  return (
    <Section id="targets" title="Financial targets" description="The assumptions behind your scorecard, journey, and projections. Change them here and every screen — and the Copilot — follows the same numbers.">
      {field("Target savings rate", "Pay-yourself-first floor used by the health score and savings nudges.", savingsRate, setSavingsRate, "%", "1")}
      {field("Emergency fund target", "Months of expenses a full emergency fund should cover (Ramsey: 3–6).", efMonths, setEfMonths, "months", "0.5")}
      {field("Expected annual return", "Long-run growth the compound projector assumes when a goal has no linked account APY.", returnPct, setReturnPct, "% / yr", "0.5")}
      <div className="s-row"><div /><div style={{ textAlign: "right" }}><button className="btn primary sm" type="button" disabled={save.isPending || !dirty} onClick={() => void onSave()}>{save.isPending ? "Applying…" : "Apply targets"}</button></div><div /></div>
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
    <Section
      id="backups"
      title="Data & backups"
      description="Your data is encrypted on this device. FinSight snapshots it before every update; you can also back up on demand and restore a snapshot."
    >
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
              <span className={`chip ${integrityOk ? "positive" : "warning"}`}>
                {integrityOk ? "Healthy" : (health?.integrityStatus || "Unknown")}
              </span>
            </div>
            <div />
          </div>

          {health && health.startupWarnings.length > 0 && (
            <div className="card" style={{ borderColor: "var(--negative)", marginBottom: 12 }}>
              <div className="label" style={{ color: "var(--negative)" }}>Some background updates didn't finish</div>
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
              <button className="btn ghost sm" type="button" style={{ marginTop: 10 }} disabled={cancelRestore.isPending} onClick={async () => { try { await cancelRestore.mutateAsync(); toast.success("Restore cancelled"); } catch (e) { toast.error("Could not cancel", { description: userErrorMessage(e) }); } }}>Cancel staged restore</button>
            </div>
          )}

          <div className="s-row">
            <div>
              <div className="label">Storage</div>
              <div className="desc">Database {fmtBytes(health?.dbBytes ?? 0)} · write-ahead log {fmtBytes(health?.walBytes ?? 0)}.</div>
            </div>
            <div className="row row-sm" style={{ justifyContent: "flex-end" }}>
              <button className="btn primary sm" type="button" disabled={backup.isPending} onClick={async () => { try { const b = await backup.mutateAsync(); toast.success("Backup created", { description: b.name }); } catch (e) { toast.error("Backup failed", { description: userErrorMessage(e) }); } }}>{backup.isPending ? "Backing up…" : "Back up now"}</button>
            </div>
            <div />
          </div>

          <div style={{ marginTop: 14 }}>
            <div className="label" style={{ marginBottom: 8 }}>Snapshots</div>
            {(!health || health.backups.length === 0) ? (
              <div className="muted" style={{ fontSize: 13 }}>No backups yet. One is created automatically before each app update.</div>
            ) : (
              <div className="tbl" role="table">
                {health.backups.map((b) => (
                  <div key={b.path} className="row" role="row" style={{ alignItems: "center", justifyContent: "space-between", padding: "8px 0", borderBottom: "1px solid var(--line)" }}>
                    <div style={{ minWidth: 0 }}>
                      <div className="mono" style={{ fontSize: 12.5, overflow: "hidden", textOverflow: "ellipsis" }}>{b.name.replace(/^data\.backup-/, "").replace(/\.sqlcipher$/, "")}</div>
                      <div className="muted" style={{ fontSize: 11.5 }}>{fmtWhen(b.createdAt)} · {fmtBytes(b.bytes)}</div>
                    </div>
                    <button className="btn ghost sm" type="button" disabled={stageRestore.isPending} onClick={async () => { try { await stageRestore.mutateAsync(b.path); toast.success("Restore staged", { description: "Restart FinSight to apply." }); } catch (e) { toast.error("Could not stage restore", { description: userErrorMessage(e) }); } }}>Restore…</button>
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
    return () => { timers.forEach((t) => clearTimeout(t)); timers.clear(); };
  }, []);

  // Forgetting is delayed 5s so the toast Undo can cancel it before the write.
  const handleForget = (m: { id: string; description: string }) => {
    setPendingForget((s) => new Set([...s, m.id]));
    const timer = setTimeout(async () => {
      forgetTimers.current.delete(m.id);
      try {
        await forgetMemory.mutateAsync(m.id);
      } catch {
        toast.error("Could not forget that memory");
      }
      setPendingForget((s) => { const n = new Set(s); n.delete(m.id); return n; });
    }, 5000);
    forgetTimers.current.set(m.id, timer);
    toast("Memory forgotten", {
      description: m.description.slice(0, 60),
      action: {
        label: "Undo",
        onClick: () => {
          const t = forgetTimers.current.get(m.id);
          if (t) { clearTimeout(t); forgetTimers.current.delete(m.id); }
          setPendingForget((s) => { const n = new Set(s); n.delete(m.id); return n; });
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
                <div className="grow" style={{ fontSize: 13.5, minWidth: 0 }}>{m.description}</div>
                <button className="btn ghost sm" type="button" onClick={() => handleForget(m)} aria-label={`Forget: ${m.description}`}>Forget</button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function Section({ id, title, description, children }: { id: string; title: string; description: string; children: React.ReactNode }) {
  return (
    <section id={`sec-${id}`}>
      <h2 className="h1" style={{ fontSize: 26 }}>{title}</h2>
      <div className="muted" style={{ marginTop: 6 }}>{description}</div>
      <div style={{ marginTop: 18 }}>{children}</div>
    </section>
  );
}

function useActiveSection(ids: readonly string[]) {
  const [active, setActive] = useState<string>(ids[0] ?? "");

  useEffect(() => {
    if (typeof IntersectionObserver === "undefined") return;
    const visibleRatios = new Map<string, number>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          visibleRatios.set(entry.target.id, entry.isIntersecting ? entry.intersectionRatio : 0);
        }
        let bestId = "";
        let bestRatio = 0;
        for (const [id, ratio] of visibleRatios) {
          if (ratio > bestRatio) {
            bestRatio = ratio;
            bestId = id;
          }
        }
        if (bestId) setActive(bestId.replace(/^sec-/, ""));
      },
      { rootMargin: "-96px 0px -60% 0px", threshold: [0, 0.25, 0.5, 0.75, 1] }
    );

    const elements = ids
      .map((id) => document.getElementById(`sec-${id}`))
      .filter((el): el is HTMLElement => el !== null);
    elements.forEach((el) => observer.observe(el));

    return () => observer.disconnect();
  }, [ids]);

  return active;
}

export default function Settings() {
  const navigate = useNavigate();
  const { data: onboarding } = useOnboardingState();
  const reset = useResetOnboarding();
  // isServerMode() reads a flag set once at boot (installHttpBackend), so
  // it's stable for the component's lifetime — safe to memoize with [].
  const serverMode = useMemo(() => isServerMode(), []);
  const sections = useMemo(
    () => (serverMode ? [...SECTIONS, SERVER_ACCOUNT_SECTION] : SECTIONS),
    [serverMode]
  );
  const sectionIds = useMemo(() => sections.map(([id]) => id), [sections]);
  const activeSection = useActiveSection(sectionIds);
  const [signingOut, setSigningOut] = useState(false);
  // Admin-only "Manage users" link in the Account section — resolved once at
  // mount from /api/auth/status. Failures are swallowed: the link simply
  // stays hidden (desktop builds never fetch this at all, serverMode guards
  // it above).
  const [isAdmin, setIsAdmin] = useState(false);
  useEffect(() => {
    if (!serverMode) return;
    let cancelled = false;
    fetchAuthStatus()
      .then((status) => {
        if (!cancelled) setIsAdmin(Boolean(status.isAdmin));
      })
      .catch(() => {
        /* link stays hidden */
      });
    return () => {
      cancelled = true;
    };
  }, [serverMode]);

  const handleSignOut = async () => {
    setSigningOut(true);
    try {
      await logout();
    } catch (error) {
      // The client still drops to the login screen below — the session
      // cookie may already be invalid/expired server-side either way.
      toast.error("Sign out request failed", { description: userErrorMessage(error) });
    } finally {
      setSigningOut(false);
      window.dispatchEvent(new CustomEvent("finsight:auth-required"));
    }
  };

  const { theme, density, accent, privacy, setTheme, setDensity, setAccent, setPrivacy } = useTweaks();
  const setCurrencyMutation = useSetCurrency();
  const exportJson = useExportJson();
  const exportCsv = useExportCsv();
  const { data: currentCurrency = "USD" } = useDefaultCurrency();
  const { data: autoCategorizeEnabled = true } = useAutoCategorizeEnabled();
  const setAutoCategorizeMutation = useSetAutoCategorizeEnabled();

  const { data: currentProvider } = useCompletionProvider();
  const setProvider = useSetCompletionProvider();
  const saveKey = useSaveProviderApiKey();
  const testProvider = useTestCompletionProvider();
  const triggerCategorize = useTriggerCategorize();

  const { data: sfStatus } = useSimpleFinStatus();
  const { data: sfConnections = [] } = useSimpleFinConnections();
  const disconnectSf = useDisconnectSimpleFin();
  const purgeSf = usePurgeSimpleFinData();
  const deleteConnection = useDeleteSimpleFinConnection();
  const { data: sfSyncSettings } = useSimpleFinSyncSettings();
  const setSfSyncSettings = useSetSimpleFinSyncSettings();

  const [sfDialogOpen, setSfDialogOpen] = useState(false);
  const [deleteAllOpen, setDeleteAllOpen] = useState(false);
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
  const [resetError, setResetError] = useState<string | null>(null);

  const modelsConfig = useMemo<CompletionProviderConfig | null>(() => {
    if (selectedKind !== "ollama") return null;
    return { kind: "ollama", base_url: ollamaUrl, model: ollamaModel };
  }, [ollamaModel, ollamaUrl, selectedKind]);
  const { data: ollamaModels = [] } = useListProviderModels(modelsConfig);

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

  const reRunOnboarding = async () => {
    setResetError(null);
    try {
      await reset.mutateAsync();
      navigate("/onboarding");
    } catch (error) {
      setResetError(userErrorMessage(error, "Could not reopen setup."));
    }
  };

  const handleTestConnection = async () => {
    const config = buildConfig();
    if (!config) return;
    try {
      // An empty field tests the STORED key (the one the runtime actually
      // uses) — the backend falls back to the keychain — so a green result
      // with a blank field genuinely reflects the live configuration.
      const result = await testProvider.mutateAsync({ config, apiKey: apiKey.trim() || undefined });
      setTestResult(result);
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
    <div className="screen screen-settings">
      <div className="day-hdr">
        <div>
          <div className="eyebrow">Settings</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Make it yours.</h1>
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "200px 1fr", gap: 56 }}>
        <nav style={{ position: "sticky", top: 16, alignSelf: "start", display: "flex", flexDirection: "column", gap: 4 }}>
          {sections.map(([id, label]) => <a key={id} href={`#sec-${id}`} className={`nav-item${activeSection === id ? " active" : ""}`}>{label}</a>)}
        </nav>

        <div style={{ display: "flex", flexDirection: "column", gap: 56 }}>
          <Section id="profile" title="Profile" description="Who this setup is for and how to restart it.">
            <div className="s-row">
              <div><div className="label">Onboarding</div><div className="desc">Completed: {onboarding?.completion_marked ? "yes" : "no"}</div></div>
              <div>{resetError && <div className="muted">{resetError}</div>}</div>
              <button className="btn sm" type="button" onClick={() => void reRunOnboarding()}>Re-run onboarding</button>
            </div>
            <div className="s-row"><div><div className="label">Account name</div><div className="desc">This desktop app is configured for your local FinSight profile.</div></div><div className="muted">FinSight desktop</div><div /></div>
          </Section>

          {serverMode && (
            <Section id="account" title="Account" description="You're signed in on this FinSight server.">
              {isAdmin && (
                <div className="s-row">
                  <div>
                    <div className="label">Users</div>
                    <div className="desc">Add or remove accounts on this server.</div>
                  </div>
                  <div />
                  <button className="btn outline sm" type="button" onClick={() => navigate("/settings/users")}>
                    Manage users
                  </button>
                </div>
              )}
              <div className="s-row">
                <div>
                  <div className="label">Sign out</div>
                  <div className="desc">End your session on this device. You'll need your password to sign back in.</div>
                </div>
                <div />
                <button className="btn outline sm" type="button" disabled={signingOut} onClick={() => void handleSignOut()}>
                  {signingOut ? "Signing out…" : "Sign out"}
                </button>
              </div>
            </Section>
          )}

          <FinancialTargetsSection />
          <PhilosophySection />

          <DataBackupsSection />

          <Section id="privacy" title="Privacy & data" description="Keep control of your data and what appears on-screen.">
            <div className="s-row">
              <div><div className="label">Privacy mode</div><div className="desc">Blur displayed amounts when you are sharing your screen or want extra discretion.</div></div>
              <div className="muted">Shortcut: ⌘.</div>
              <Tog checked={privacy} onChange={setPrivacy} />
            </div>
            <div className="s-row">
              <div><div className="label">Export data</div><div className="desc">Download the full dataset as JSON or CSV whenever you want a local backup.</div></div>
              <div className="row row-sm wrap"><button className="btn sm" type="button" onClick={async () => { try { await exportJson.mutateAsync(); toast.success("File saved"); } catch (error) { toast.error("Export failed", { description: userErrorMessage(error, "Try exporting again from the desktop app.") }); } }}>Export as JSON</button><button className="btn sm" type="button" onClick={async () => { try { await exportCsv.mutateAsync(); toast.success("File saved"); } catch (error) { toast.error("Export failed", { description: userErrorMessage(error, "Try exporting again from the desktop app.") }); } }}>Export as CSV</button></div>
              <div />
            </div>
            <div className="s-row">
              <div><div className="label">Delete all data</div><div className="desc">Permanently remove every account, transaction, balance, budget, goal, insight, and agent memory from this device. Your AI provider settings and API keys are kept. This cannot be undone.</div></div>
              <div><button className="btn danger sm" type="button" onClick={() => setDeleteAllOpen(true)}>Delete all data</button></div>
              <div />
            </div>
          </Section>

          <Section id="agent" title="Agent" description="Control what the agent does automatically, and what it remembers.">
            <div className="s-row"><div><div className="label">Auto-categorize new transactions</div><div className="desc">Automatically categorize transactions after each import or sync, using your configured AI provider.</div></div><div className="muted">{autoCategorizeEnabled ? "Currently on" : "Currently off"}</div><Tog checked={autoCategorizeEnabled} onChange={(value) => setAutoCategorizeMutation.mutate(value)} /></div>
            <AgentMemoryPanel />
            <div className="card tight" style={{ marginTop: 12 }}>
              <div className="row row-sm" style={{ alignItems: "flex-start", gap: 8 }}>
                <span aria-hidden style={{ fontSize: 15 }}>🔒</span>
                <div className="muted" style={{ fontSize: 12.5, lineHeight: 1.5 }}>
                  <strong style={{ color: "var(--ink)" }}>What leaves your device.</strong> When auto-categorize is on and you use a <em>cloud</em> AI provider (OpenAI-compatible or Anthropic), the merchant description and amount of each <em>uncategorized</em> transaction are sent to that provider to pick a category. Balances, account numbers, and totals are never sent. Transaction reference numbers, and the names of people in e-transfers, are redacted before sending. Choose a local <strong>Ollama</strong> provider to keep everything on this machine, or turn auto-categorize off to categorize manually.
                </div>
              </div>
            </div>
          </Section>

          <Section id="provider" title="AI Provider" description="Choose where categorization and forecasting run.">
            {!providerPanelOpen ? (
              <div className="card tight">
                <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 16 }}>
                  <div className="muted">{providerDisplayName(currentProvider)}</div>
                  <button className="btn sm" type="button" onClick={() => setProviderPanelOpen(true)}>{currentProvider && currentProvider.kind !== "unconfigured" ? "Edit" : "Configure"}</button>
                </div>
              </div>
            ) : (
              <div className="card">
                <div className="toolbar" style={{ marginBottom: 18 }}>
                  <button className={selectedKind === "ollama" ? "on" : ""} type="button" onClick={() => { setSelectedKind("ollama"); setApiKey(""); }}>Ollama</button>
                  <button className={selectedKind === "openai_compat" ? "on" : ""} type="button" onClick={() => { setSelectedKind("openai_compat"); setApiKey(""); }}>Cloud</button>
                  <button className={selectedKind === "anthropic" ? "on" : ""} type="button" onClick={() => { setSelectedKind("anthropic"); setApiKey(""); }}>Anthropic</button>
                </div>

                {selectedKind === "ollama" && <div className="stack stack-md"><label className="stack stack-xs"><span className="muted">Base URL</span><input className="control" value={ollamaUrl} onChange={(e) => setOllamaUrl(e.target.value)} /></label><label className="stack stack-xs"><span className="muted">Model</span><select className="control" value={ollamaModel} onChange={(e) => setOllamaModel(e.target.value)}>{ollamaModels.map((model) => <option key={model} value={model}>{model}</option>)}{ollamaModels.length === 0 && <option value="">Pick a model</option>}</select></label></div>}
                {selectedKind === "openai_compat" && <div className="stack stack-md"><div className="row row-sm wrap">{OPENAI_COMPAT_PRESETS.map((preset) => <button key={preset.preset} className={`btn ${selectedPreset.preset === preset.preset ? "primary" : "outline"} sm`} type="button" onClick={() => setSelectedPreset(preset)}>{preset.label}</button>)}</div><label className="stack stack-xs"><span className="muted">Model</span><input className="control" value={compatModel} onChange={(e) => setCompatModel(e.target.value)} placeholder="e.g. gpt-4o-mini" /></label><label className="stack stack-xs"><span className="muted">API key</span><input className="control" type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-…" /></label></div>}
                {selectedKind === "anthropic" && <div className="stack stack-md"><label className="stack stack-xs"><span className="muted">Model</span><input className="control" value={anthropicModel} onChange={(e) => setAnthropicModel(e.target.value)} /></label><label className="stack stack-xs"><span className="muted">API key</span><input className="control" type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-ant-…" /></label></div>}

                {testResult && <div className="muted" style={{ marginTop: 14 }}>{testResult.ok ? `Connected — ${testResult.latency_ms}ms` : testResult.error}</div>}
                {saveError && <div className="muted" style={{ marginTop: 14 }}>{saveError}</div>}

                <div className="row row-sm wrap" style={{ marginTop: 18 }}>
                  <button className="btn sm" type="button" onClick={() => void handleTestConnection()}>Test connection</button>
                  <button className="btn primary sm" type="button" onClick={() => void handleSave()}>Save</button>
                  <button className="btn ghost sm" type="button" onClick={() => { setProviderPanelOpen(false); setTestResult(null); }}>Cancel</button>
                  <button className="btn outline sm" type="button" onClick={() => triggerCategorize.mutate()}>Re-categorize all</button>
                </div>
              </div>
            )}
          </Section>

          <Section id="appearance" title="Appearance" description="Theme, density, accent, and currency.">
            <div className="s-row"><div><div className="label">Theme</div><div className="desc">Switch between dark and light modes.</div></div><div className="toolbar"><button className={theme === "dark" ? "on" : ""} type="button" onClick={() => setTheme("dark")}>Dark</button><button className={theme === "light" ? "on" : ""} type="button" onClick={() => setTheme("light")}>Light</button></div><div /></div>
            <div className="s-row"><div><div className="label">Density</div><div className="desc">Use cozy spacing or fit more on screen.</div></div><div className="toolbar"><button className={density === "cozy" ? "on" : ""} type="button" onClick={() => setDensity("cozy")}>Cozy</button><button className={density === "compact" ? "on" : ""} type="button" onClick={() => setDensity("compact")}>Compact</button></div><div /></div>
            <div className="s-row"><div><div className="label">Accent</div><div className="desc">Pick the accent used in hero states and active controls.</div></div><div className="row row-sm wrap">{(Object.entries(ACCENTS) as [AccentId, { hex: string }][]).map(([id, value]) => <button key={id} type="button" aria-label={id} onClick={() => setAccent(id)} style={{ width: 28, height: 28, borderRadius: 999, background: value.hex, border: accent === id ? "2px solid var(--ink)" : "1px solid var(--line)" }} />)}</div><div /></div>
            <div className="s-row"><div><div className="label">Currency</div><div className="desc">Used for all money formatting in the app.</div></div><div><select className="control" value={currentCurrency} onChange={(e) => setCurrencyMutation.mutate(e.target.value)} style={{ maxWidth: 140 }}>{CURRENCIES.map((currency) => <option key={currency} value={currency}>{currency}</option>)}</select></div><div /></div>
          </Section>

          <Section id="connections" title="Connections" description="Bank feeds and background sync via SimpleFin.">
            <div className="s-row"><div><div className="label">SimpleFin</div><div className="desc">Connect or add institutions and import synced transactions.</div></div><div className="muted">{sfStatus?.configured ? "Connected" : "Not connected"}</div><button className="btn sm" type="button" onClick={() => setSfDialogOpen(true)}>{sfStatus?.configured ? "Add connection" : "Set up SimpleFin"}</button></div>
            {sfStatus?.configured && <div className="s-row"><div><div className="label">Background sync</div><div className="desc">Choose how often the desktop app checks for updates.</div></div><div className="toolbar">{[0, 60, 180, 360, 720].map((minutes) => <button key={minutes} className={(sfSyncSettings?.backgroundSyncIntervalMinutes ?? 360) === minutes ? "on" : ""} type="button" onClick={() => setSfSyncSettings.mutate({ backgroundSyncEnabled: minutes > 0, backgroundSyncIntervalMinutes: minutes })}>{minutes === 0 ? "Off" : minutes === 60 ? "1 hour" : minutes === 180 ? "3 hours" : minutes === 360 ? "6 hours" : "12 hours"}</button>)}</div><div /></div>}
            {sfConnections.map((connection) => <div key={connection.id} className="s-row"><div><div className="label">{connection.label || connection.orgName || "SimpleFin connection"}</div><div className="desc">{connection.status}{connection.lastSyncedAt ? ` · last synced ${new Date(connection.lastSyncedAt).toLocaleString()}` : ""}</div></div><div className="muted">Connected</div><button className="btn ghost sm" type="button" onClick={() => deleteConnection.mutate(connection.id, { onSuccess: () => toast.success("Connection removed"), onError: () => toast.error("Failed to remove connection") })}>Remove</button></div>)}
            {sfConnections.length > 0 && <div className="s-row"><div><div className="label">Disconnect all</div><div className="desc">Remove all stored SimpleFin credentials.</div></div><div /><button className="btn outline sm" type="button" onClick={() => disconnectSf.mutate(undefined, { onSuccess: () => toast.success("All SimpleFin credentials removed"), onError: () => toast.error("Failed to remove credentials") })}>Disconnect all</button></div>}
            {sfConnections.length > 0 && <div className="s-row"><div><div className="label">Remove imported SimpleFin data</div><div className="desc">Deletes SimpleFin accounts, synced transactions, connection records, and stored credentials. Manual accounts are not touched.</div></div><div /><button className="btn outline sm" type="button" disabled={purgeSf.isPending} onClick={() => {
              if (!confirm("Remove all imported SimpleFin accounts and transactions from this local profile? This keeps manual data but requires reconnecting SimpleFin.")) return;
              purgeSf.mutate(undefined, {
                onSuccess: (summary) => toast.success("Imported SimpleFin data removed", { description: `${summary.accountsDeleted} accounts and ${summary.transactionsDeleted} transactions removed.` }),
                onError: () => toast.error("Failed to remove imported SimpleFin data"),
              });
            }}>{purgeSf.isPending ? "Removing..." : "Remove imported data"}</button></div>}
            <SimpleFinDialog open={sfDialogOpen} onClose={() => setSfDialogOpen(false)} />
          </Section>

          <Section id="notifications" title="Notifications" description="Choose what you're notified about, when it stays quiet, and how much detail shows.">
            <NotificationPolicySettings />
            <PushNotificationSettings />
          </Section>

          <Section id="keyboard" title="Keyboard" description="Shortcuts available across the app.">
            <div className="s-row"><div><div className="label">Command palette</div><div className="desc">Jump to screens and quick actions.</div></div><div><kbd className="tok">⌘K</kbd></div><div /></div>
            <div className="s-row"><div><div className="label">Privacy mode</div><div className="desc">Toggle amount blurring instantly.</div></div><div><kbd className="tok">⌘.</kbd></div><div /></div>
          </Section>

          <Section id="about" title="About" description="Version info and development helpers.">
            <div className="s-row"><div><div className="label">App version</div><div className="desc">Desktop runtime build information.</div></div><div className="muted">FinSight desktop · local build</div><div /></div>
          </Section>
        </div>
      </div>
      <DeleteAllDataDialog open={deleteAllOpen} onClose={() => setDeleteAllOpen(false)} />
    </div>
  );
}
