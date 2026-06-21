import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useAccounts } from "../api/hooks/accounts";
import {
  useResetOnboarding,
  useClearSampleData,
  useSeedDevDemo,
  useOnboardingState,
} from "../api/hooks/onboarding";
import {
  useCompletionProvider,
  useSetCompletionProvider,
  useSaveProviderApiKey,
  useTestCompletionProvider,
  useTriggerCategorize,
  useListProviderModels,
} from "../api/hooks/agent";
import { useDefaultCurrency, useSetCurrency, useExportJson, useExportCsv, useNotificationsEnabled, useSetNotificationsEnabled } from "../api/hooks/settings";
import { useSimpleFinStatus, useDisconnectSimpleFin } from "../api/hooks/simplefin";
import SimpleFinDialog from "./onboarding/SimpleFinDialog";
import { useTweaks, ACCENTS, type AccentId } from "../state/tweaks";
import type { CompletionProviderConfig } from "../api/client";
import { userErrorMessage } from "../utils/runtime";
import Button from "../components/Button";
import Card from "../components/Card";
import Input from "../components/Input";
import Select from "../components/Select";
import Swatch from "../components/Swatch";

type ProviderKind = "ollama" | "openai_compat" | "anthropic" | null;

const OPENAI_COMPAT_PRESETS: { label: string; preset: string; base_url: string }[] = [
  { label: "OpenAI", preset: "openai", base_url: "https://api.openai.com/v1" },
  { label: "OpenRouter", preset: "openrouter", base_url: "https://openrouter.ai/api/v1" },
  { label: "Google", preset: "google", base_url: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { label: "Custom", preset: "custom", base_url: "" },
];

const CURRENCIES = ["USD","EUR","GBP","CAD","AUD","JPY","CHF","NZD","SGD","HKD"];

function providerDisplayName(cfg: CompletionProviderConfig): string {
  switch (cfg.kind) {
    case "ollama":
      return `Ollama (${cfg.model})`;
    case "openai_compat":
      return `${cfg.preset} (${cfg.model})`;
    case "anthropic":
      return `Anthropic (${cfg.model})`;
    case "unconfigured":
      return "Not configured";
  }
}

export default function Settings() {
  const navigate = useNavigate();
  const reset = useResetOnboarding();
  const clearSample = useClearSampleData();
  const seedDemo = useSeedDevDemo();
  const { data: accounts = [] } = useAccounts();
  const { data: onboarding } = useOnboardingState();
  const hasSample = accounts.some((a) => a.source === "sample");
  const [resetError, setResetError] = useState<string | null>(null);
  const [clearError, setClearError] = useState<string | null>(null);
  const [sfDialogOpen, setSfDialogOpen] = useState(false);
  const { data: sfStatus } = useSimpleFinStatus();
  const disconnectSf = useDisconnectSimpleFin();

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
  const { data: notificationsEnabled = true } = useNotificationsEnabled();
  const setNotificationsMutation = useSetNotificationsEnabled();

  const { data: currentProvider } = useCompletionProvider();
  const setProvider = useSetCompletionProvider();
  const saveKey = useSaveProviderApiKey();
  const testProvider = useTestCompletionProvider();
  const triggerCategorize = useTriggerCategorize();
  const { data: ollamaModels = [] } = useListProviderModels(
    selectedKind === "ollama"
      ? { kind: "ollama", base_url: ollamaUrl, model: ollamaModel }
      : null
  );

  // Pre-populate the provider panel from the saved configuration when it opens.
  useEffect(() => {
    if (!providerPanelOpen || !currentProvider) return;
    switch (currentProvider.kind) {
      case "ollama":
        setSelectedKind("ollama");
        setOllamaUrl(currentProvider.base_url);
        setOllamaModel(currentProvider.model);
        break;
      case "openai_compat": {
        setSelectedKind("openai_compat");
        const preset = OPENAI_COMPAT_PRESETS.find((p) => p.preset === currentProvider.preset);
        if (preset) {
          setSelectedPreset(preset);
        } else {
          setSelectedPreset({
            label: currentProvider.preset,
            preset: currentProvider.preset,
            base_url: currentProvider.base_url,
          });
        }
        setCompatModel(currentProvider.model);
        break;
      }
      case "anthropic":
        setSelectedKind("anthropic");
        setAnthropicModel(currentProvider.model);
        break;
      case "unconfigured":
        setSelectedKind(null);
        break;
    }
  }, [providerPanelOpen, currentProvider]);

  async function reRunOnboarding() {
    if (!confirm("This will re-open the welcome wizard. Your existing accounts, transactions, and categories are kept.")) return;
    setResetError(null);
    try {
      await reset.mutateAsync();
      navigate("/onboarding");
    } catch (err) {
      setResetError(userErrorMessage(err, "Could not reopen setup. Try again from the desktop app."));
    }
  }

  async function replaceSampleData() {
    if (!confirm("This will permanently delete the Mira & Adam sample accounts and their transactions. Anything you added manually or imported is kept.")) return;
    setClearError(null);
    try {
      await clearSample.mutateAsync();
      navigate("/onboarding");
    } catch (err) {
      setClearError(userErrorMessage(err, "Could not clear sample data. Try again from the desktop app."));
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
      setTestResult({ ok: false, latency_ms: 0, error: userErrorMessage(err, "Connection failed. Check the provider settings and try again.") });
    }
  }

  async function handleSave() {
    if (!selectedKind) return;
    const config = buildConfig();
    if (!config) return;
    setSaveError(null);
    try {
      if (apiKey && selectedKind !== "ollama") {
        const pid = selectedKind === "anthropic" ? "anthropic" : selectedPreset.preset;
        await saveKey.mutateAsync({ providerId: pid, key: apiKey });
      }
      await setProvider.mutateAsync(config);
      setProviderPanelOpen(false);
    } catch (err) {
      setSaveError(userErrorMessage(err, "Could not save provider settings. Check the fields and try again."));
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

      <section className="section-stack" style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Onboarding</h2>
        <p style={{ marginBottom: 12 }}>
          Completed: <strong>{onboarding?.completion_marked ? "yes" : "no"}</strong>
        </p>
        {resetError && <p role="alert" className="err">{resetError}</p>}
        <Button variant="default" onClick={reRunOnboarding}>Re-run onboarding</Button>
      </section>

      {hasSample && (
        <section className="section-stack" style={{ marginBottom: 32 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Sample data</h2>
          <p style={{ marginBottom: 12 }}>
            You're currently looking at the Mira &amp; Adam sample household. Replace it with your own when you're ready.
          </p>
          {clearError && <p role="alert" className="err">{clearError}</p>}
          <Button variant="danger" onClick={replaceSampleData}>Replace sample data with my own</Button>
        </section>
      )}

      <section className="section-stack" style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>AI Provider</h2>
        {!providerPanelOpen ? (
          <Card className="stack stack-md" tight>
            <p className="muted" style={{ margin: 0 }}>
              {currentProvider && currentProvider.kind !== "unconfigured"
                ? `Configured — ${providerDisplayName(currentProvider)}.`
                : "Not configured — categories won't be assigned automatically."}
            </p>
            <Button variant="default" onClick={() => setProviderPanelOpen(true)}>
              {currentProvider && currentProvider.kind !== "unconfigured" ? "Edit" : "Configure"}
            </Button>
          </Card>
        ) : (
          <Card className="stack stack-md" tight>
            <div className="row-sm wrap">
              {(["ollama", "openai_compat", "anthropic"] as ProviderKind[]).map((k) => (
                <Button
                  key={k!}
                  variant={selectedKind === k ? "primary" : "outline"}
                  size="sm"
                  onClick={() => { setSelectedKind(k); setApiKey(""); }}
                  aria-pressed={selectedKind === k}
                >
                  {k === "ollama" ? "Ollama" : k === "anthropic" ? "Anthropic" : "Cloud"}
                </Button>
              ))}
            </div>

            {selectedKind === "ollama" && (
              <div className="stack stack-md">
                <Input
                  label="Base URL"
                  value={ollamaUrl}
                  onChange={(e) => setOllamaUrl(e.target.value)}
                />
                <Select
                  label="Model"
                  value={ollamaModel}
                  onChange={(e) => setOllamaModel(e.target.value)}
                >
                  {ollamaModels.map((m: string) => <option key={m} value={m}>{m}</option>)}
                </Select>
              </div>
            )}

            {selectedKind === "openai_compat" && (
              <div className="stack stack-md">
                <div className="row-sm wrap">
                  {OPENAI_COMPAT_PRESETS.map((p) => (
                    <Button
                      key={p.preset}
                      variant={selectedPreset.preset === p.preset ? "primary" : "outline"}
                      size="sm"
                      onClick={() => setSelectedPreset(p)}
                      aria-pressed={selectedPreset.preset === p.preset}
                    >
                      {p.label}
                    </Button>
                  ))}
                </div>
                <Input
                  label="Model"
                  value={compatModel}
                  onChange={(e) => setCompatModel(e.target.value)}
                  placeholder="e.g. gpt-4o-mini"
                />
                <Input
                  label="API Key"
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="sk-…"
                />
              </div>
            )}

            {selectedKind === "anthropic" && (
              <div className="stack stack-md">
                <Input
                  label="Model"
                  value={anthropicModel}
                  onChange={(e) => setAnthropicModel(e.target.value)}
                />
                <Input
                  label="API Key"
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="sk-ant-…"
                />
              </div>
            )}

            {testResult && (
              <p style={{ color: testResult.ok ? "var(--success, green)" : "var(--error, red)", margin: 0 }}>
                {testResult.ok ? `✓ Connected — ${testResult.latency_ms}ms` : `✗ ${testResult.error}`}
              </p>
            )}

            {saveError && <p role="alert" className="err">{saveError}</p>}

            <div className="row-sm wrap">
              <Button
                variant="default"
                onClick={handleTestConnection}
                disabled={!selectedKind || testProvider.isPending}
                loading={testProvider.isPending}
              >
                Test connection
              </Button>
              <Button
                variant="primary"
                onClick={handleSave}
                disabled={!selectedKind || setProvider.isPending}
                loading={setProvider.isPending}
              >
                Save
              </Button>
              <Button variant="ghost" onClick={() => { setProviderPanelOpen(false); setTestResult(null); }}>
                Cancel
              </Button>
            </div>

            <div style={{ paddingTop: 12, borderTop: "1px solid var(--hairline)" }}>
              <Button
                variant="outline"
                onClick={() => triggerCategorize.mutate()}
                disabled={triggerCategorize.isPending}
                loading={triggerCategorize.isPending}
              >
                Re-categorize all
              </Button>
            </div>
          </Card>
        )}
      </section>

      <section className="section-stack" style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 16 }}>Appearance</h2>

        <div className="stack stack-lg">
          <div className="row-md wrap" style={{ alignItems: "center" }}>
            <span style={{ width: 80, fontSize: 13 }} className="muted">Theme</span>
            <div className="toolbar" style={{ display: "inline-flex" }}>
              <button className={theme === "light" ? "on" : ""} aria-pressed={theme === "light"} onClick={() => setTheme("light")}>Light</button>
              <button className={theme === "dark" ? "on" : ""} aria-pressed={theme === "dark"} onClick={() => setTheme("dark")}>Dark</button>
            </div>
          </div>

          <div className="row-md wrap" style={{ alignItems: "center" }}>
            <span style={{ width: 80, fontSize: 13 }} className="muted">Density</span>
            <div className="toolbar" style={{ display: "inline-flex" }}>
              <button className={density === "cozy" ? "on" : ""} aria-pressed={density === "cozy"} onClick={() => setDensity("cozy")}>Cozy</button>
              <button className={density === "compact" ? "on" : ""} aria-pressed={density === "compact"} onClick={() => setDensity("compact")}>Compact</button>
            </div>
          </div>

          <div className="row-md wrap" style={{ alignItems: "center" }}>
            <span style={{ width: 80, fontSize: 13 }} className="muted">Accent</span>
            <div className="row-sm">
              {(Object.entries(ACCENTS) as [AccentId, { hex: string; ink: string }][]).map(([id, { hex }]) => (
                <Swatch
                  key={id}
                  color={hex}
                  selected={accent === id}
                  onClick={() => setAccent(id)}
                  label={id}
                />
              ))}
            </div>
          </div>

          <div className="row-md wrap" style={{ alignItems: "center" }}>
            <span style={{ width: 80, fontSize: 13 }} className="muted">Currency</span>
            <Select
              aria-label="Currency"
              value={currentCurrency}
              onChange={(e) => {
                setCurrencyMutation.mutate(e.target.value, {
                  onError: (err) => toast.error("Currency update failed — " + (err instanceof Error ? err.message : "unknown error")),
                });
              }}
              style={{ width: "auto", minWidth: 100 }}
            >
              {CURRENCIES.map((c) => (
                <option key={c} value={c}>{c}</option>
              ))}
            </Select>
          </div>

          <div className="row-md wrap" style={{ alignItems: "center" }}>
            <span style={{ width: 80, fontSize: 13 }} className="muted">Notifications</span>
            <div className="toolbar" style={{ display: "inline-flex" }}>
              <button
                className={notificationsEnabled ? "on" : ""}
                aria-pressed={notificationsEnabled}
                onClick={() => setNotificationsMutation.mutate(true)}
              >
                On
              </button>
              <button
                className={!notificationsEnabled ? "on" : ""}
                aria-pressed={!notificationsEnabled}
                onClick={() => setNotificationsMutation.mutate(false)}
              >
                Off
              </button>
            </div>
            <span className="muted" style={{ fontSize: 12 }}>Budget alerts and bill reminders</span>
          </div>
        </div>
      </section>

      <section className="section-stack" style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Export data</h2>
        <p className="muted" style={{ marginBottom: 14, fontSize: 14 }}>
          Download your complete data as JSON or a transaction CSV.
        </p>
        <div className="row-sm">
          <Button
            variant="default"
            loading={exportJson.isPending}
            disabled={exportJson.isPending}
            onClick={async () => {
              try {
                await exportJson.mutateAsync();
                toast.success("File saved");
              } catch (err) {
                toast.error("Export failed", {
                  description: userErrorMessage(err, "Try exporting again from the desktop app."),
                });
              }
            }}
          >
            {exportJson.isPending ? "Exporting…" : "Export as JSON"}
          </Button>
          <Button
            variant="default"
            loading={exportCsv.isPending}
            disabled={exportCsv.isPending}
            onClick={async () => {
              try {
                await exportCsv.mutateAsync();
                toast.success("File saved");
              } catch (err) {
                toast.error("Export failed", {
                  description: userErrorMessage(err, "Try exporting again from the desktop app."),
                });
              }
            }}
          >
            {exportCsv.isPending ? "Exporting…" : "Export as CSV"}
          </Button>
        </div>
      </section>

      {import.meta.env.DEV && (
        <section style={{ marginBottom: 32 }}>
          <div className="eyebrow" style={{ marginBottom: 12 }}>Development</div>
          <Card tone="accent" style={{ opacity: 0.9 }} className="stack stack-md">
            <div className="stack stack-xs">
              <strong>Load demo data</strong>
              <p className="muted" style={{ fontSize: 13, margin: 0 }}>
                Seeds the "Mira & Adam" prototype dataset — 6 accounts, 6 months of transactions,
                goals, assets, liabilities, and budgets. Replaces any existing sample data. Dev only.
              </p>
            </div>
            <Button
              variant="default"
              loading={seedDemo.isPending}
              disabled={seedDemo.isPending}
              onClick={async () => {
                try {
                  const s = await seedDemo.mutateAsync();
                  toast.success(`Demo data loaded — ${s.transactions_created} transactions`);
                } catch (err) {
                  toast.error("Could not load demo data", {
                    description: userErrorMessage(err, "Open FinSight with the desktop runtime and try again."),
                  });
                }
              }}
            >
              {seedDemo.isPending ? "Loading…" : "Load demo data"}
            </Button>
          </Card>
        </section>
      )}

      <section className="section-stack">
        <div className="eyebrow" style={{ marginBottom: 14 }}>Bank connections</div>
        <Card className="stack stack-md">
          <div className="row-md" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <span>SimpleFin: {sfStatus?.configured ? "Connected" : "Not connected"}</span>
            {sfStatus?.configured ? (
              <Button
                variant="default"
                onClick={() => {
                  disconnectSf.mutate(undefined, {
                    onSuccess: () => toast.success("SimpleFin credentials removed"),
                    onError: () => toast.error("Failed to remove credentials"),
                  });
                }}
              >
                Reset credentials
              </Button>
            ) : (
              <Button variant="default" onClick={() => setSfDialogOpen(true)}>
                Set up SimpleFin
              </Button>
            )}
          </div>
        </Card>
        <SimpleFinDialog open={sfDialogOpen} onClose={() => setSfDialogOpen(false)} />
      </section>

      <section className="section-stack">
        <div className="eyebrow" style={{ marginBottom: 14 }}>Keyboard shortcuts</div>
        <Card tight className="stack">
          {[
            { key: "⌘K", label: "Open command palette" },
            { key: "⌘.", label: "Toggle privacy mode" },
          ].map(({ key, label }, i, arr) => (
            <div
              key={key}
              className="row-md"
              style={{
                alignItems: "center",
                padding: "10px 0",
                borderBottom: i < arr.length - 1 ? "1px solid var(--line)" : "none",
              }}
            >
              <kbd className="num" style={{
                fontFamily: "var(--mono)", fontSize: 13, padding: "3px 8px",
                background: "var(--surface-2)", border: "1px solid var(--line)",
                borderRadius: 5, color: "var(--ink)", minWidth: 36, textAlign: "center",
              }}>
                {key}
              </kbd>
              <span style={{ fontSize: 14 }}>{label}</span>
            </div>
          ))}
        </Card>
      </section>
    </div>
  );
}
