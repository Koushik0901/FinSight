import { useEffect, useState, type FormEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/openapiClient";
import { unwrap } from "../../api/openapiClient";
import type { OllamaProbeResult } from "../../api/openapiClient";
import { useMarkOnboardingComplete } from "../../api/hooks/onboarding";
import {
  useSetCompletionProvider,
  useSaveProviderApiKey,
  useTestCompletionProvider,
  useListProviderModels,
} from "../../api/hooks/agent";
import { isBackendAvailable, userErrorMessage } from "../../utils/runtime";
import Button from "../../components/Button";
import Card from "../../components/Card";
import Input from "../../components/Input";
import Select from "../../components/Select";
import { Cpu, House } from "../../components/Icons";

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
  const [baseUrl, setBaseUrl] = useState("http://ollama:11434");
  const [probedBaseUrl, setProbedBaseUrl] = useState("http://ollama:11434");
  const [completionModel, setCompletionModel] = useState("");
  const { data: probe, refetch, isFetching } = useQuery<OllamaProbeResult>({
    queryKey: ["ollama-probe", probedBaseUrl],
    queryFn: async () => {
      return unwrap(api.probeOllama(probedBaseUrl));
    },
    staleTime: 0,
    // probe_ollama is a plain RPC executed server-side, so it works over HTTP —
    // gating it on the (narrowed) isTauriRuntime() left the local-AI onboarding
    // path permanently stuck on "could not find Ollama" in server/PWA/shell
    // mode, with the "refresh" button inert because refetch() no-ops on a
    // disabled query.
    enabled: path === "local" && isBackendAvailable(),
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
    path === "local" ? { kind: "ollama", base_url: probedBaseUrl, model: completionModel } : null
  );
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    // Treat a partial/older/mock response as "unreachable" instead of letting
    // onboarding crash while indexing a missing models array.
    const first = Array.isArray(probe?.models) ? probe.models[0] : undefined;
    if (first && !completionModel) setCompletionModel(first);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [probe]);

  async function finishWithOllama() {
    if (!probe?.reachable || !completionModel) return;
    setActionError(null);
    try {
      await setProvider.mutateAsync({ kind: "ollama", base_url: probedBaseUrl, model: completionModel });
      await api.saveLlmProvider({ kind: "ollama", base_url: probedBaseUrl, completion_model: completionModel, embedding_model: "nomic-embed-text" });
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(userErrorMessage(err, "Could not save the provider. Check your FinSight server connection and try again."));
    }
  }

  function checkOllamaConnection(e: FormEvent) {
    e.preventDefault();
    const next = baseUrl.trim().replace(/\/+$/, "");
    if (!next) {
      setActionError("Enter the Ollama URL that your FinSight server can reach.");
      return;
    }
    setActionError(null);
    setCompletionModel("");
    if (next === probedBaseUrl) {
      void refetch();
    } else {
      setProbedBaseUrl(next);
    }
  }

  async function handleCloudTestAndSave(e: FormEvent) {
    e.preventDefault();
    setActionError(null);
    setTestResult(null);
    const isAnthropic = selectedPreset.preset === "anthropic";
    const config = isAnthropic
      ? { kind: "anthropic" as const, model: cloudModel }
      : { kind: "openai_compat" as const, preset: selectedPreset.preset, base_url: selectedPreset.base_url, model: cloudModel };
    try {
      const r = await testProvider.mutateAsync({ config, apiKey: apiKey || undefined });
      setTestResult({ ok: r.ok, latency_ms: r.latency_ms, error: r.error ?? null });
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

  async function finishWithoutProvider() {
    setActionError(null);
    try {
      await setProvider.mutateAsync({ kind: "unconfigured" });
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(userErrorMessage(err, "Could not finish setup. Check your FinSight server connection and try again."));
    }
  }

  // Local categorization is the default. Ollama/cloud are optional fallbacks
  // for ambiguous merchants, not prerequisites for using FinSight.
  if (!path) {
    return (
      <div className="step-agent onb-split">
        <div className="onb-left">
          <div className="num-step">004 · Categorization</div>
          <h1>Local first. Review when it matters.</h1>
          <p className="lead">FinSight already includes a trained merchant model. It categorizes routine transactions on your server, without an API key or an AI provider.</p>
          <Card className="onb-local-model-card stack stack-md">
            <div className="onb-local-model-head">
              <div className="onb-local-model-mark" aria-hidden="true">FT</div>
              <div>
                <div className="onb-provider-title" style={{ fontWeight: 700 }}>FastText merchant model</div>
                <div className="muted" style={{ fontSize: 13 }}>Built in · local · zero provider setup</div>
              </div>
              <span className="chip is-good">Default</span>
            </div>
            <p className="muted onb-local-model-copy">Rules catch known patterns first. FastText handles familiar merchants next. Only uncertain transactions need your review or an optional model fallback.</p>
            <div className="onb-routing-pipeline" aria-label="Categorization pipeline">
              <span className="onb-routing-node">Rules</span>
              <span className="onb-routing-arrow" aria-hidden="true">→</span>
              <span className="onb-routing-node is-active">FastText</span>
              <span className="onb-routing-arrow" aria-hidden="true">→</span>
              <span className="onb-routing-node is-muted">Optional AI</span>
            </div>
          </Card>
          <div className="onb-local-actions">
            <Button variant="primary" onClick={finishWithoutProvider}>Continue with local categorization →</Button>
            <span className="muted" style={{ fontSize: 12.5 }}>You can add a fallback later in Settings.</span>
          </div>
          <div className="onb-provider-choice-grid">
            <Button
              className="onb-provider-choice"
              variant="outline"
              onClick={() => setPath("local")}
            >
              <div className="stack stack-xs" style={{ textAlign: "left" }}>
                <div className="onb-provider-title" style={{ fontWeight: 700 }}><House width={16} height={16} /> Add Ollama fallback</div>
                <div className="muted" style={{ fontSize: 13 }}>Keep uncertain cases on your own network.</div>
              </div>
            </Button>
            <Button
              className="onb-provider-choice"
              variant="outline"
              onClick={() => setPath("cloud")}
            >
              <div className="stack stack-xs" style={{ textAlign: "left" }}>
                <div className="onb-provider-title" style={{ fontWeight: 700 }}><Cpu width={16} height={16} /> Add cloud fallback</div>
                <div className="muted" style={{ fontSize: 13 }}>Use a provider only for the long tail.</div>
              </div>
            </Button>
          </div>
          {actionError && <p role="alert" className="err">{actionError}</p>}
        </div>

        <div className="onb-right">
          <Card className="onb-routing-card stack stack-md">
            <div className="eyebrow"><span className="dot" />How categorization works</div>
            <h3 className="h3">Quiet automation, visible decisions.</h3>
            <div className="onb-routing-steps">
              <div className="onb-routing-step"><span>01</span><div><strong>Match locally</strong><small>Rules and FastText run on your server.</small></div></div>
              <div className="onb-routing-step"><span>02</span><div><strong>Surface uncertainty</strong><small>Low-confidence items stay easy to review.</small></div></div>
              <div className="onb-routing-step"><span>03</span><div><strong>Learn from corrections</strong><small>Your choices improve future suggestions.</small></div></div>
            </div>
            <span className="chip">Private by default</span>
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
          <div className="num-step">004 · Optional cloud fallback</div>
          <h1>Add a cloud fallback.</h1>
          <p className="lead">FinSight will keep using its local merchant model first, then send only the uncertain cases to this provider.</p>
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
          <form onSubmit={(e) => void handleCloudTestAndSave(e)}>
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
              <p className={testResult.ok ? "onb-provider-status is-good" : "onb-provider-status is-bad"} role="status" aria-live="polite">
                {testResult.ok ? `✓ Connected — ${testResult.latency_ms}ms` : `✗ ${testResult.error}`}
              </p>
            )}
            {actionError && <p role="alert" className="err">{actionError}</p>}
            <div className="actions row-sm wrap">
              <Button
                variant="primary"
                type="submit"
                disabled={!cloudModel || testProvider.isPending}
                loading={testProvider.isPending}
              >
                Test &amp; Save →
              </Button>
              <Button variant="default" type="button" onClick={() => setPath(null)}>← Back</Button>
              <Button variant="ghost" type="button" onClick={finishWithoutProvider}>Keep local only →</Button>
            </div>
          </form>
        </div>
        <div className="onb-right">
          <Card className="stack stack-md">
            <div className="eyebrow"><span className="dot" />Security</div>
            <div className="muted" style={{ fontSize: 13.5, lineHeight: 1.5 }}>
              API keys are encrypted inside your signed-in FinSight profile on the server. Your financial data stays on your self-hosted server; only the prompts needed for categorization are sent to the provider you configure.
            </div>
          </Card>
        </div>
      </div>
    );
  }

  // Self-hosted Ollama path
  if (isFetching && !probe) {
    return (
      <div className="step-agent onb-split">
        <div className="onb-left">
          <div className="num-step">004 · Optional Ollama fallback</div>
          <h1>Checking for Ollama…</h1>
          <p className="lead">Asking your FinSight server to connect to {probedBaseUrl}.</p>
        </div>
        <div className="onb-right">
          <Card role="status" aria-live="polite">Waiting for runtime probe…</Card>
        </div>
      </div>
    );
  }

  const normalizedBaseUrl = baseUrl.trim().replace(/\/+$/, "");
  const urlNeedsCheck = normalizedBaseUrl !== probedBaseUrl;

  if (!probe?.reachable || urlNeedsCheck) {
    return (
      <div className="step-agent onb-split">
        <div className="onb-left">
          <div className="num-step">004 · Optional Ollama fallback</div>
          <h1>{urlNeedsCheck ? "Check this Ollama server." : "Add an Ollama fallback."}</h1>
          <p className="lead">
            {urlNeedsCheck
              ? "Test the address from FinSight before choosing a model."
              : "FinSight could not reach Ollama at this address. Install it on your server host or use another server-reachable URL. Local FastText categorization will continue to work without it."}
          </p>
          <form onSubmit={(e) => checkOllamaConnection(e)}>
            <Input
              label="Ollama URL (as reached by the FinSight server)"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="http://ollama:11434"
              hint="With the provided Docker Compose service, use http://ollama:11434."
            />
            {actionError && <p role="alert" className="err">{actionError}</p>}
            <div className="actions row-sm wrap">
              <Button variant="primary" type="submit">Check connection</Button>
              <a className="btn" href="https://ollama.com" target="_blank" rel="noreferrer">Ollama setup guide ↗</a>
              <Button variant="default" type="button" onClick={() => setPath(null)}>← Back</Button>
              <Button variant="ghost" type="button" onClick={finishWithoutProvider}>Keep local only →</Button>
            </div>
          </form>
        </div>
        <div className="onb-right">
          <Card className="stack stack-sm">
            <div className="eyebrow">Server-side setup</div>
            <span className="chip">1. Run Ollama on your server or network</span>
            <span className="chip">2. Pull a completion model</span>
            <span className="chip">3. Enter the server-reachable URL</span>
          </Card>
        </div>
      </div>
    );
  }

  return (
    <div className="step-agent onb-split">
      <div className="onb-left">
        <div className="num-step">004 · Optional Ollama fallback</div>
        <h1>Ollama is ready as a fallback.</h1>
        <p className="lead">Connected through your FinSight server at {probedBaseUrl}. Pick a model for the uncertain cases FastText cannot classify locally.</p>
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
          <Button variant="ghost" onClick={finishWithoutProvider}>Keep local only →</Button>
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
