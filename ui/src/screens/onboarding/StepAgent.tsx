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
    enabled: path === "local",
  });

  // Cloud path state
  const [selectedPreset, setSelectedPreset] = useState<CloudPreset>(CLOUD_PRESETS[0]);
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
      setActionError(err instanceof Error ? err.message : "Something went wrong.");
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
      await setProvider.mutateAsync(config);
      if (apiKey) {
        await saveKey.mutateAsync({ providerId: isAnthropic ? "anthropic" : selectedPreset.preset, key: apiKey });
      }
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function skipForLater() {
    setActionError(null);
    try {
      await setProvider.mutateAsync({ kind: "unconfigured" });
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  // Initial two-path choice
  if (!path) {
    return (
      <div className="step-agent">
        <h2>How do you want to power AI categorization?</h2>
        <div style={{ display: "flex", gap: 16, marginBottom: 24, flexWrap: "wrap" }}>
          <button onClick={() => setPath("local")} style={{ flex: 1, minWidth: 160, padding: "20px 16px" }}>
            <div style={{ fontWeight: 700, marginBottom: 4 }}>🏠 Local (Ollama)</div>
            <div style={{ fontSize: 13, color: "var(--text-2)" }}>Install-free if already running.</div>
          </button>
          <button onClick={() => setPath("cloud")} style={{ flex: 1, minWidth: 160, padding: "20px 16px" }}>
            <div style={{ fontWeight: 700, marginBottom: 4 }}>☁ Cloud provider</div>
            <div style={{ fontSize: 13, color: "var(--text-2)" }}>OpenAI, Anthropic, OpenRouter, etc.</div>
          </button>
        </div>
        {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
        <button className="tertiary" onClick={skipForLater}>Configure later →</button>
      </div>
    );
  }

  // Cloud path
  if (path === "cloud") {
    return (
      <div className="step-agent">
        <h2>Cloud provider</h2>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
          {CLOUD_PRESETS.map((p) => (
            <button
              key={p.preset}
              onClick={() => { setSelectedPreset(p); setCloudModel(""); setApiKey(""); setTestResult(null); }}
              aria-pressed={selectedPreset.preset === p.preset}
            >
              {p.label}
            </button>
          ))}
        </div>
        <label style={{ display: "block", marginBottom: 8 }}>
          Model
          <input value={cloudModel} onChange={(e) => setCloudModel(e.target.value)} placeholder="e.g. gpt-4o-mini" style={{ display: "block", width: "100%" }} />
        </label>
        <label style={{ display: "block", marginBottom: 8 }}>
          API Key
          <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-…" style={{ display: "block", width: "100%" }} />
        </label>
        {testResult && (
          <p style={{ color: testResult.ok ? "var(--success, green)" : "var(--error, red)" }}>
            {testResult.ok ? `✓ Connected — ${testResult.latency_ms}ms` : `✗ ${testResult.error}`}
          </p>
        )}
        {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
        <div className="actions" style={{ display: "flex", gap: 8 }}>
          <button className="primary" onClick={handleCloudTestAndSave} disabled={!cloudModel || testProvider.isPending}>
            Test &amp; Save →
          </button>
          <button onClick={() => setPath(null)}>← Back</button>
          <button className="tertiary" onClick={skipForLater}>Configure later →</button>
        </div>
      </div>
    );
  }

  // Local (Ollama) path
  if (isFetching && !probe) {
    return <div className="step-agent"><p>Checking for Ollama…</p></div>;
  }

  if (!probe?.reachable) {
    return (
      <div className="step-agent">
        <h2>Set up Ollama</h2>
        <p>
          We couldn't find Ollama. Download it from{" "}
          <a href="#" onClick={(e) => { e.preventDefault(); openUrl("https://ollama.com"); }}>ollama.com</a>.
        </p>
        {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
        <div className="actions">
          <button onClick={() => openUrl("https://ollama.com").catch(() => {})}>Install Ollama →</button>
          <button onClick={() => refetch()}>I just installed it — refresh</button>
          <button onClick={() => setPath(null)}>← Back</button>
          <button className="tertiary" onClick={skipForLater}>Configure later →</button>
        </div>
      </div>
    );
  }

  return (
    <div className="step-agent">
      <h2>Set up Ollama</h2>
      <p>Ollama is running. Pick a completion model.</p>
      <label>
        Completion model
        <select value={completionModel} onChange={(e) => setCompletionModel(e.target.value)}>
          {(ollamaModels as string[]).map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </label>
      {!probe.has_nomic_embed && (
        <p className="warning">
          <code>nomic-embed-text</code> isn't installed. Run{" "}
          <code>ollama pull nomic-embed-text</code>, then <button onClick={() => refetch()}>Refresh</button>.
        </p>
      )}
      {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
      <div className="actions">
        <button className="primary" onClick={finishWithOllama} disabled={!completionModel}>
          Use Ollama →
        </button>
        <button onClick={() => setPath(null)}>← Back</button>
        <button className="tertiary" onClick={skipForLater}>Configure later →</button>
      </div>
    </div>
  );
}
