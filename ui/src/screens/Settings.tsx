import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import {
  useResetOnboarding,
  useClearSampleData,
  useOnboardingState,
} from "../api/hooks/onboarding";
import {
  useSetCompletionProvider,
  useSaveProviderApiKey,
  useTestCompletionProvider,
  useTriggerCategorize,
  useListProviderModels,
} from "../api/hooks/agent";
import { useDefaultCurrency, useSetCurrency, useExportJson, useExportCsv } from "../api/hooks/settings";
import { useTweaks, ACCENTS, type AccentId } from "../state/tweaks";
import type { CompletionProviderConfig } from "../api/client";

type ProviderKind = "ollama" | "openai_compat" | "anthropic" | null;

const OPENAI_COMPAT_PRESETS: { label: string; preset: string; base_url: string }[] = [
  { label: "OpenAI", preset: "openai", base_url: "https://api.openai.com/v1" },
  { label: "OpenRouter", preset: "openrouter", base_url: "https://openrouter.ai/api/v1" },
  { label: "Google", preset: "google", base_url: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { label: "Custom", preset: "custom", base_url: "" },
];

export default function Settings() {
  const navigate = useNavigate();
  const reset = useResetOnboarding();
  const clearSample = useClearSampleData();
  const { data: accounts = [] } = useAccounts();
  const { data: onboarding } = useOnboardingState();
  const hasSample = accounts.some((a) => a.source === "sample");
  const [resetError, setResetError] = useState<string | null>(null);
  const [clearError, setClearError] = useState<string | null>(null);

  // AI Provider panel state
  const [providerPanelOpen, setProviderPanelOpen] = useState(false);
  const [selectedKind, setSelectedKind] = useState<ProviderKind>(null);
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [ollamaModel, setOllamaModel] = useState("");
  const [selectedPreset, setSelectedPreset] = useState(OPENAI_COMPAT_PRESETS[0]!);
  const [compatModel, setCompatModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [anthropicModel, setAnthropicModel] = useState("claude-3-5-haiku-latest");
  const [testResult, setTestResult] = useState<{ ok: boolean; latency_ms: number; error: string | null } | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const { theme, density, accent, setTheme, setDensity, setAccent } = useTweaks();
  const setCurrencyMutation = useSetCurrency();
  const exportJson = useExportJson();
  const exportCsv = useExportCsv();
  const { data: currentCurrency = "USD" } = useDefaultCurrency();

  const setProvider = useSetCompletionProvider();
  const saveKey = useSaveProviderApiKey();
  const testProvider = useTestCompletionProvider();
  const triggerCategorize = useTriggerCategorize();
  const { data: ollamaModels = [] } = useListProviderModels(
    selectedKind === "ollama"
      ? { kind: "ollama", base_url: ollamaUrl, model: ollamaModel }
      : null
  );

  async function reRunOnboarding() {
    if (!confirm("This will re-open the welcome wizard. Your existing accounts, transactions, and categories are kept.")) return;
    setResetError(null);
    try {
      await reset.mutateAsync();
      navigate("/onboarding");
    } catch (err) {
      setResetError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function replaceSampleData() {
    if (!confirm("This will permanently delete the Mira & Adam sample accounts and their transactions. Anything you added manually or imported is kept.")) return;
    setClearError(null);
    try {
      await clearSample.mutateAsync();
      navigate("/onboarding");
    } catch (err) {
      setClearError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function handleTestConnection() {
    if (!selectedKind) return;
    const config = buildConfig();
    if (!config) return;
    setTestResult(null);
    try {
      const r = await testProvider.mutateAsync({ config, apiKey: apiKey || undefined });
      setTestResult(r);
    } catch (err) {
      setTestResult({ ok: false, latency_ms: 0, error: err instanceof Error ? err.message : "Connection failed" });
    }
  }

  async function handleSave() {
    if (!selectedKind) return;
    const config = buildConfig();
    if (!config) return;
    setSaveError(null);
    try {
      await setProvider.mutateAsync(config);
      if (apiKey && selectedKind !== "ollama") {
        const pid = selectedKind === "anthropic" ? "anthropic" : selectedPreset.preset;
        await saveKey.mutateAsync({ providerId: pid, key: apiKey });
      }
      setProviderPanelOpen(false);
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : "Failed to save provider.");
    }
  }

  function buildConfig(): CompletionProviderConfig | null {
    switch (selectedKind) {
      case "ollama": return { kind: "ollama", base_url: ollamaUrl, model: ollamaModel };
      case "openai_compat": return { kind: "openai_compat", preset: selectedPreset.preset, base_url: selectedPreset.base_url, model: compatModel };
      case "anthropic": return { kind: "anthropic", model: anthropicModel };
      default: return null;
    }
  }

  return (
    <div className="screen-settings">
      <h1 style={{ fontSize: 32, fontWeight: 600, marginTop: 0, marginBottom: 24 }}>Settings</h1>

      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Onboarding</h2>
        <p style={{ marginBottom: 12 }}>
          Completed: <strong>{onboarding?.completion_marked ? "yes" : "no"}</strong>
        </p>
        {resetError && <p role="alert" style={{ color: "var(--error, red)", marginBottom: 8 }}>{resetError}</p>}
        <button onClick={reRunOnboarding}>Re-run onboarding</button>
      </section>

      {hasSample && (
        <section style={{ marginBottom: 32 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Sample data</h2>
          <p style={{ marginBottom: 12 }}>
            You're currently looking at the Mira &amp; Adam sample household. Replace it with your own when you're ready.
          </p>
          {clearError && <p role="alert" style={{ color: "var(--error, red)", marginBottom: 8 }}>{clearError}</p>}
          <button onClick={replaceSampleData} className="danger">Replace sample data with my own</button>
        </section>
      )}

      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>AI Provider</h2>
        {!providerPanelOpen ? (
          <div>
            <p style={{ marginBottom: 12, color: "var(--text-2)" }}>
              Not configured — categories won't be assigned automatically.
            </p>
            <button onClick={() => setProviderPanelOpen(true)}>Configure</button>
          </div>
        ) : (
          <div style={{ border: "1px solid var(--hairline)", borderRadius: 8, padding: 16 }}>
            {/* Provider type row */}
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
              {(["ollama", "openai_compat", "anthropic"] as ProviderKind[]).map((k) => (
                <button
                  key={k!}
                  onClick={() => { setSelectedKind(k); setApiKey(""); }}
                  style={{ fontWeight: selectedKind === k ? 700 : 400 }}
                  aria-pressed={selectedKind === k}
                >
                  {k === "ollama" ? "Ollama" : k === "anthropic" ? "Anthropic" : "Cloud"}
                </button>
              ))}
            </div>

            {selectedKind === "ollama" && (
              <div>
                <label style={{ display: "block", marginBottom: 8 }}>
                  Base URL
                  <input value={ollamaUrl} onChange={(e) => setOllamaUrl(e.target.value)} style={{ display: "block", width: "100%" }} />
                </label>
                <label style={{ display: "block", marginBottom: 8 }}>
                  Model
                  <select value={ollamaModel} onChange={(e) => setOllamaModel(e.target.value)} style={{ display: "block", width: "100%" }}>
                    {ollamaModels.map((m: string) => <option key={m} value={m}>{m}</option>)}
                  </select>
                </label>
              </div>
            )}

            {selectedKind === "openai_compat" && (
              <div>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 8 }}>
                  {OPENAI_COMPAT_PRESETS.map((p) => (
                    <button key={p.preset} onClick={() => setSelectedPreset(p)} aria-pressed={selectedPreset.preset === p.preset}>
                      {p.label}
                    </button>
                  ))}
                </div>
                <label style={{ display: "block", marginBottom: 8 }}>
                  Model
                  <input value={compatModel} onChange={(e) => setCompatModel(e.target.value)} placeholder="e.g. gpt-4o-mini" style={{ display: "block", width: "100%" }} />
                </label>
                <label style={{ display: "block", marginBottom: 8 }}>
                  API Key
                  <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-…" style={{ display: "block", width: "100%" }} />
                </label>
              </div>
            )}

            {selectedKind === "anthropic" && (
              <div>
                <label style={{ display: "block", marginBottom: 8 }}>
                  Model
                  <input value={anthropicModel} onChange={(e) => setAnthropicModel(e.target.value)} style={{ display: "block", width: "100%" }} />
                </label>
                <label style={{ display: "block", marginBottom: 8 }}>
                  API Key
                  <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-ant-…" style={{ display: "block", width: "100%" }} />
                </label>
              </div>
            )}

            {testResult && (
              <p style={{ color: testResult.ok ? "var(--success, green)" : "var(--error, red)", marginBottom: 8 }}>
                {testResult.ok ? `✓ Connected — ${testResult.latency_ms}ms` : `✗ ${testResult.error}`}
              </p>
            )}

            {saveError && <p role="alert" style={{ color: "var(--error, red)", marginBottom: 8 }}>{saveError}</p>}

            <div style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
              <button onClick={handleTestConnection} disabled={!selectedKind || testProvider.isPending}>
                Test connection
              </button>
              <button className="primary" onClick={handleSave} disabled={!selectedKind || setProvider.isPending}>
                Save
              </button>
              <button onClick={() => { setProviderPanelOpen(false); setTestResult(null); }}>
                Cancel
              </button>
            </div>

            <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid var(--hairline)" }}>
              <button onClick={() => triggerCategorize.mutate()} disabled={triggerCategorize.isPending}>
                Re-categorize all
              </button>
            </div>
          </div>
        )}
      </section>

      {/* §12c: Appearance section */}
      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 16 }}>Appearance</h2>

        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <span style={{ width: 80, fontSize: 13, color: "var(--ink-mute)" }}>Theme</span>
            <div className="toolbar" style={{ display: "inline-flex" }}>
              <button className={theme === "light" ? "on" : ""} onClick={() => setTheme("light")}>Light</button>
              <button className={theme === "dark" ? "on" : ""} onClick={() => setTheme("dark")}>Dark</button>
            </div>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <span style={{ width: 80, fontSize: 13, color: "var(--ink-mute)" }}>Density</span>
            <div className="toolbar" style={{ display: "inline-flex" }}>
              <button className={density === "cozy" ? "on" : ""} onClick={() => setDensity("cozy")}>Cozy</button>
              <button className={density === "compact" ? "on" : ""} onClick={() => setDensity("compact")}>Compact</button>
            </div>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <span style={{ width: 80, fontSize: 13, color: "var(--ink-mute)" }}>Accent</span>
            <div style={{ display: "flex", gap: 8 }}>
              {(Object.entries(ACCENTS) as [AccentId, { hex: string; ink: string }][]).map(([id, { hex }]) => (
                <button
                  key={id}
                  aria-label={id}
                  onClick={() => setAccent(id)}
                  style={{
                    width: 24, height: 24, borderRadius: 999, background: hex, cursor: "pointer",
                    border: accent === id ? "2px solid var(--ink)" : "2px solid transparent",
                    padding: 0,
                  }}
                />
              ))}
            </div>
          </div>

          {/* §12b: Currency */}
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <span style={{ width: 80, fontSize: 13, color: "var(--ink-mute)" }}>Currency</span>
            <select
              value={currentCurrency}
              onChange={(e) => {
                setCurrencyMutation.mutate(e.target.value);
              }}
              style={{ background: "var(--surface-2)", border: "1px solid var(--line-2)",
                borderRadius: 7, padding: "6px 10px", fontSize: 14, color: "var(--ink)", outline: "none" }}
            >
              {["USD","EUR","GBP","CAD","AUD","JPY","CHF","NZD","SGD","HKD"].map((c) => (
                <option key={c} value={c}>{c}</option>
              ))}
            </select>
          </div>
        </div>
      </section>

      {/* §12a: Data export section */}
      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Export data</h2>
        <p style={{ marginBottom: 14, color: "var(--ink-mute)", fontSize: 14 }}>
          Download your complete data as JSON or a transaction CSV.
        </p>
        <div style={{ display: "flex", gap: 10 }}>
          <button
            disabled={exportJson.isPending}
            onClick={async () => {
              try {
                await exportJson.mutateAsync();
                toast.success("File saved");
              } catch (err) {
                toast.error("Export failed — " + (err instanceof Error ? err.message : "unknown error"));
              }
            }}
          >
            {exportJson.isPending ? "Exporting…" : "Export as JSON"}
          </button>
          <button
            disabled={exportCsv.isPending}
            onClick={async () => {
              try {
                await exportCsv.mutateAsync();
                toast.success("File saved");
              } catch (err) {
                toast.error("Export failed — " + (err instanceof Error ? err.message : "unknown error"));
              }
            }}
          >
            {exportCsv.isPending ? "Exporting…" : "Export as CSV"}
          </button>
        </div>
      </section>
    </div>
  );
}
