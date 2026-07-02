import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useResetOnboarding, useOnboardingState } from "../api/hooks/onboarding";
import {
  useCompletionProvider,
  useSetCompletionProvider,
  useSaveProviderApiKey,
  useTestCompletionProvider,
  useTriggerCategorize,
  useListProviderModels,
} from "../api/hooks/agent";
import { useDefaultCurrency, useSetCurrency, useExportJson, useExportCsv, useNotificationsEnabled, useSetNotificationsEnabled } from "../api/hooks/settings";
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
  ["privacy", "Privacy & data"],
  ["provider", "AI Provider"],
  ["appearance", "Appearance"],
  ["connections", "Connections"],
  ["notifications", "Notifications"],
  ["keyboard", "Keyboard"],
  ["about", "About"],
] as const;
const SECTION_IDS = SECTIONS.map(([id]) => id);

function providerDisplayName(cfg: CompletionProviderConfig | undefined) {
  if (!cfg || cfg.kind === "unconfigured") return "Not configured";
  if (cfg.kind === "ollama") return `Configured — Ollama (${cfg.model})`;
  if (cfg.kind === "anthropic") return `Configured — Anthropic (${cfg.model})`;
  return `Configured — ${cfg.preset} (${cfg.model})`;
}

function Tog({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return <span className={`tog${checked ? " on" : ""}`} role="switch" aria-checked={checked} tabIndex={0} onClick={() => onChange(!checked)} onKeyDown={(e) => e.key === "Enter" && onChange(!checked)} />;
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
  const activeSection = useActiveSection(SECTION_IDS);

  const { theme, density, accent, privacy, setTheme, setDensity, setAccent, setPrivacy } = useTweaks();
  const setCurrencyMutation = useSetCurrency();
  const exportJson = useExportJson();
  const exportCsv = useExportCsv();
  const { data: currentCurrency = "USD" } = useDefaultCurrency();
  const { data: notificationsEnabled = true } = useNotificationsEnabled();
  const setNotificationsMutation = useSetNotificationsEnabled();

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
      const result = await testProvider.mutateAsync({ config, apiKey: apiKey || undefined });
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
      if (apiKey && selectedKind && selectedKind !== "ollama") {
        const providerId = selectedKind === "anthropic" ? "anthropic" : selectedPreset.preset;
        await saveKey.mutateAsync({ providerId, key: apiKey });
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
          {SECTIONS.map(([id, label]) => <a key={id} href={`#sec-${id}`} className={`nav-item${activeSection === id ? " active" : ""}`}>{label}</a>)}
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

          <Section id="notifications" title="Notifications" description="Control reminders and nudges.">
            <div className="s-row"><div><div className="label">Notifications enabled</div><div className="desc">Budget alerts, recurring reminders, and daily prompts.</div></div><div className="muted">{notificationsEnabled ? "Currently on" : "Currently off"}</div><Tog checked={notificationsEnabled} onChange={(value) => setNotificationsMutation.mutate(value)} /></div>
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
    </div>
  );
}
