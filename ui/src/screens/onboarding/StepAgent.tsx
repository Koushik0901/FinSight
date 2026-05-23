import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands } from "../../api/client";
import type { OllamaProbeResult } from "../../api/client";
import { useMarkOnboardingComplete } from "../../api/hooks/onboarding";

interface Props { onDone: () => void; }

export default function StepAgent({ onDone }: Props) {
  const [baseUrl] = useState("http://localhost:11434");
  const { data: probe, refetch, isFetching } = useQuery<OllamaProbeResult>({
    queryKey: ["ollama-probe", baseUrl],
    queryFn: async () => {
      const result = await commands.probeOllama(baseUrl);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 0,
  });
  const [completionModel, setCompletionModel] = useState("");
  const markComplete = useMarkOnboardingComplete();
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
      const r = await commands.saveLlmProvider({
        kind: "ollama",
        base_url: baseUrl,
        completion_model: completionModel,
        embedding_model: "nomic-embed-text",
      });
      if (r.status === "error") throw new Error(r.error.message);
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function skipForLater() {
    setActionError(null);
    try {
      const r = await commands.saveLlmProvider({ kind: "unconfigured" });
      if (r.status === "error") throw new Error(r.error.message);
      await markComplete.mutateAsync();
      onDone();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  if (isFetching && !probe) {
    return <div className="step-agent"><p>Checking for Ollama…</p></div>;
  }

  if (!probe?.reachable) {
    return (
      <div className="step-agent">
        <h2>Set up the agent</h2>
        <p>
          We couldn't find a local model. FinSight uses{" "}
          <a href="#" onClick={(e) => { e.preventDefault(); openUrl("https://ollama.com"); }}>
            Ollama
          </a>{" "}
          for private agent features.
        </p>
        {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
        <div className="actions">
          <button onClick={() => openUrl("https://ollama.com")}>Install Ollama →</button>
          <button onClick={() => refetch()}>I just installed it — refresh</button>
          <button className="tertiary" onClick={skipForLater}>Configure later →</button>
        </div>
      </div>
    );
  }

  return (
    <div className="step-agent">
      <h2>Set up the agent</h2>
      <p>Ollama is running. Pick a completion model.</p>
      <label>
        Completion model
        <select value={completionModel} onChange={(e) => setCompletionModel(e.target.value)}>
          {probe.models.map((m) => (
            <option key={m} value={m}>{m}</option>
          ))}
        </select>
      </label>
      {!probe.has_nomic_embed && (
        <p className="warning">
          <code>nomic-embed-text</code> isn't installed. Run{" "}
          <code>ollama pull nomic-embed-text</code> in your terminal, then{" "}
          <button onClick={() => refetch()}>Refresh</button>.
        </p>
      )}
      {actionError && <p role="alert" style={{ color: "var(--error, red)" }}>{actionError}</p>}
      <div className="actions">
        <button
          className="primary"
          onClick={finishWithOllama}
          disabled={!probe.has_nomic_embed || !completionModel}
        >
          Use Ollama →
        </button>
        <button className="tertiary" onClick={skipForLater}>Skip for now</button>
      </div>
    </div>
  );
}
