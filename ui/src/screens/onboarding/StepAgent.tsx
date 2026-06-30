import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands } from "../../api/client";
import type { OllamaProbeResult } from "../../api/client";
import { useMarkOnboardingComplete } from "../../api/hooks/onboarding";
import {
  useSetCompletionProvider,
  useSaveProviderApiKey,
  useTestCompletionProvider,
  useListProviderModels,
} from "../../api/hooks/agent";
import { isTauriRuntime, userErrorMessage } from "../../utils/runtime";
import Button from "../../components/Button";
import Card from "../../components/Card";
import Input from "../../components/Input";
import Select from "../../components/Select";

interface Props { onDone: () => void; }

type Path = null | "local" | "cloud";
type CloudPreset = { label: string; preset: string; base_url: string };

const CLOUD_PRESETS: CloudPreset[] = [
  { label: "OpenAI", preset: "openai", base_url: "https://api.openai.com/v1" },
  { label: "OpenRouter", preset: "openrouter", base_url: "https://openrouter.ai/api/v1" },
  { label: "Anthropic", preset: "anthropic", base_url: "" },
  { label: "Google", preset: "google", base_url: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { label: "Custom", preset: "custom", base_url: "" },
];

export default function StepAgent({ onDone }: Props) {
  const [path, setPath] = useState<Path>(null);

  // Ollama path state
  const [baseUrl] = useState("http://localhost:11434");
  const [completionModel, setCompletionModel] = useState("");
  const { data: probe, refetch, isFetching } = useQuery<OllamaProbeResult>({
    queryKey: ["ollama-probe", baseUrl],
    queryFn: async () => {
      const result = await commands.probeOllama(baseUrl);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 0,
    enabled: path === "local" && isTauriRuntime(),
  });

  // Cloud path state
  const [selectedPreset, setSelectedPreset] = useState<CloudPreset>(CLOUD_PRESETS[0]!);
  const [cloudModel, setCloudModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [testResult, setTestResult] = useState<{ ok: boolean; latency_ms: number; error: string | null } | null>(null);

  const markComplete = useMarkOnboardingComplete();
  const setProvider = useSetCompletionProvider();
  const saveKey = useSaveProviderApiKey();
  const testProvider = useTestCompletionProvider();
  const { data: ollamaModels = [] } = useListProviderModels(
    path === "local" ? { kind: "ollama", base_url: baseUrl, model: completionModel } : null
  );
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    const first = probe?.models[0];
    if (first && !completionModel) setCompletionModel(first);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [probe]);

  async function finishWithOllama() {
    if (!probe?.reachable || !completionModel) return;
    setActionError(null);
    try {
      await setProvider.mutateAsync({ kind: "ollama", base_url: baseUrl, model: completionModel });
      await commands.saveLlmProvider({ kind: "ollama", base_url: baseUrl, completion_model: completionModel, embedding_model: "nomic-embed-text" });
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(userErrorMessage(err, "Could not save the local provider. Try again from the desktop app."));
    }
  }

  async function handleCloudTestAndSave() {
    setActionError(null);
    setTestResult(null);
    const isAnthropic = selectedPreset.preset === "anthropic";
    const config = isAnthropic
      ? { kind: "anthropic" as const, model: cloudModel }
      : { kind: "openai_compat" as const, preset: selectedPreset.preset, base_url: selectedPreset.base_url, model: cloudModel };
    try {
      const r = await testProvider.mutateAsync({ config, apiKey: apiKey || undefined });
      setTestResult(r);
      if (!r.ok) return;
      if (apiKey) {
        await saveKey.mutateAsync({ providerId: isAnthropic ? "anthropic" : selectedPreset.preset, key: apiKey });
      }
      await setProvider.mutateAsync(config);
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(userErrorMessage(err, "Could not test or save this provider. Check the settings and try again."));
    }
  }

  async function skipForLater() {
    setActionError(null);
    try {
      await setProvider.mutateAsync({ kind: "unconfigured" });
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(userErrorMessage(err, "Could not finish setup. Try again from the desktop app."));
    }
  }

  // Initial two-path choice
  if (!path) {
    return (
      <div className="step-agent onb-split">
        <div className="onb-left">
          <div className="num-step">004 · AI setup</div>
          <h1>Choose how to power AI categorization.</h1>
          <p className="lead">Pick local Ollama for private on-device inference or connect a cloud provider.</p>
          <div className="row-md wrap" style={{ marginBottom: 24 }}>
            <Button
              variant="outline"
              onClick={() => setPath("local")}
              style={{ flex: 1, minWidth: 160, padding: "20px 16px", justifyContent: "flex-start" }}
            >
              <div className="stack stack-xs" style={{ textAlign: "left" }}>
                <div style={{ fontWeight: 700 }}>🏠 Local (Ollama)</div>
                <div className="muted" style={{ fontSize: 13 }}>Install-free if already running.</div>
              </div>
            </Button>
            <Button
              variant="outline"
              onClick={() => setPath("cloud")}
              style={{ flex: 1, minWidth: 160, padding: "20px 16px", justifyContent: "flex-start" }}
            >
              <div className="stack stack-xs" style={{ textAlign: "left" }}>
                <div style={{ fontWeight: 700 }}>☁ Cloud provider</div>
                <div className="muted" style={{ fontSize: 13 }}>OpenAI, Anthropic, OpenRouter, etc.</div>
              </div>
            </Button>
          </div>
          {actionError && <p role="alert" className="err">{actionError}</p>}
          <Button variant="ghost" onClick={skipForLater}>Configure later →</Button>
        </div>

        <div className="onb-right">
          <Card className="stack stack-md">
            <div className="eyebrow"><span className="dot" />What happens next</div>
            <div className="h3">FinSight will:</div>
            <div className="stack stack-xs muted" style={{ fontSize: 13.5 }}>
              <div>• Categorize transactions automatically</div>
              <div>• Mark low-confidence items for quick review</div>
              <div>• Learn from your corrections over time</div>
            </div>
            <span className="chip">You can change providers later in Settings</span>
          </Card>
        </div>
      </div>
    );
  }

  // Cloud path
  if (path === "cloud") {
    return (
      <div className="step-agent onb-split">
        <div className="onb-left">
          <div className="num-step">004 · Cloud provider</div>
          <h1>Connect a cloud model.</h1>
          <p className="lead">Choose a provider, enter the model id, then test and save.</p>
          <div className="row-sm wrap" style={{ marginBottom: 16 }}>
            {CLOUD_PRESETS.map((p) => (
              <Button
                key={p.preset}
                variant={selectedPreset.preset === p.preset ? "primary" : "outline"}
                size="sm"
                onClick={() => { setSelectedPreset(p); setCloudModel(""); setApiKey(""); setTestResult(null); }}
                aria-pressed={selectedPreset.preset === p.preset}
              >
                {p.label}
              </Button>
            ))}
          </div>
          <div className="stack stack-md">
            <Input
              label="Model"
              value={cloudModel}
              onChange={(e) => setCloudModel(e.target.value)}
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
          {testResult && (
            <p style={{ color: testResult.ok ? "var(--success, green)" : "var(--error, red)" }}>
              {testResult.ok ? `✓ Connected — ${testResult.latency_ms}ms` : `✗ ${testResult.error}`}
            </p>
          )}
          {actionError && <p role="alert" className="err">{actionError}</p>}
          <div className="actions row-sm wrap">
            <Button
              variant="primary"
              onClick={handleCloudTestAndSave}
              disabled={!cloudModel || testProvider.isPending}
              loading={testProvider.isPending}
            >
              Test &amp; Save →
            </Button>
            <Button variant="default" onClick={() => setPath(null)}>← Back</Button>
            <Button variant="ghost" onClick={skipForLater}>Configure later →</Button>
          </div>
        </div>
        <div className="onb-right">
          <Card className="stack stack-md">
            <div className="eyebrow"><span className="dot" />Security</div>
            <div className="muted" style={{ fontSize: 13.5, lineHeight: 1.5 }}>
              API keys are stored in your local OS keychain. Your financial data stays local; only prompts needed for categorization are sent to the provider you configure.
            </div>
          </Card>
        </div>
      </div>
    );
  }

  // Local (Ollama) path
  if (isFetching && !probe) {
    return (
      <div className="step-agent onb-split">
        <div className="onb-left">
          <div className="num-step">004 · Local AI</div>
          <h1>Checking for Ollama…</h1>
          <p className="lead">Looking for a local model runtime on your machine.</p>
        </div>
        <div className="onb-right">
          <Card>Waiting for runtime probe…</Card>
        </div>
      </div>
    );
  }

  if (!probe?.reachable) {
    return (
      <div className="step-agent onb-split">
        <div className="onb-left">
          <div className="num-step">004 · Local AI</div>
          <h1>Set up Ollama.</h1>
          <p className="lead">
            We could not find Ollama. Download it from{" "}
            <a href="#" onClick={(e) => { e.preventDefault(); openUrl("https://ollama.com"); }}>ollama.com</a>.
          </p>
          {actionError && <p role="alert" className="err">{actionError}</p>}
          <div className="actions row-sm wrap">
            <Button variant="default" onClick={() => openUrl("https://ollama.com").catch(() => {})}>Install Ollama →</Button>
            <Button variant="default" onClick={() => refetch()}>I just installed it — refresh</Button>
            <Button variant="default" onClick={() => setPath(null)}>← Back</Button>
            <Button variant="ghost" onClick={skipForLater}>Configure later →</Button>
          </div>
        </div>
        <div className="onb-right">
          <Card className="stack stack-sm">
            <div className="eyebrow">Local stack</div>
            <span className="chip">1. Install Ollama</span>
            <span className="chip">2. Pull model</span>
            <span className="chip">3. Refresh and continue</span>
          </Card>
        </div>
      </div>
    );
  }

  return (
    <div className="step-agent onb-split">
      <div className="onb-left">
        <div className="num-step">004 · Local AI</div>
        <h1>Ollama is ready.</h1>
        <p className="lead">Pick a completion model and finish setup.</p>
        <Select
          label="Completion model"
          value={completionModel}
          onChange={(e) => setCompletionModel(e.target.value)}
        >
          {(ollamaModels as string[]).map((m) => <option key={m} value={m}>{m}</option>)}
        </Select>
        {!probe.has_nomic_embed && (
          <p className="warning">
            <code>nomic-embed-text</code> isn't installed. Run{" "}
            <code>ollama pull nomic-embed-text</code>, then{" "}
            <Button variant="text" onClick={() => refetch()}>Refresh</Button>.
          </p>
        )}
        {actionError && <p role="alert" className="err">{actionError}</p>}
        <div className="actions row-sm wrap">
          <Button variant="primary" onClick={finishWithOllama} disabled={!completionModel}>
            Use Ollama →
          </Button>
          <Button variant="default" onClick={() => setPath(null)}>← Back</Button>
          <Button variant="ghost" onClick={skipForLater}>Configure later →</Button>
        </div>
      </div>
      <div className="onb-right">
        <Card className="stack stack-sm">
          <div className="eyebrow"><span className="dot" />Detected models</div>
          {(ollamaModels as string[]).length === 0 ? (
            <div className="muted">No completion models detected yet.</div>
          ) : (
            <div className="row row-sm wrap">
              {(ollamaModels as string[]).slice(0, 8).map((model) => (
                <span key={model} className="chip">{model}</span>
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}
