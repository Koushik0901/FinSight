import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import type { AgentAnswer } from "../api/client";
import { AgentResponseRenderer } from "./AgentResponseRenderer";
import Button from "./Button";
import Drawer from "./Drawer";
import * as I from "./Icons";

export function CopilotQuickAsk({
  prompt,
  label = "Ask Copilot",
}: {
  prompt: string;
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const [question, setQuestion] = useState(prompt);
  const [answer, setAnswer] = useState<AgentAnswer | null>(null);
  const [loading, setLoading] = useState(false);

  const handleOpen = () => {
    setQuestion(prompt);
    setAnswer(null);
    setOpen(true);
  };

  const handleAsk = async () => {
    const trimmed = question.trim();
    if (!trimmed) return;
    setLoading(true);
    setAnswer(null);
    try {
      const raw = await invoke<AgentAnswer>("ask_agent", {
        question: trimmed,
        mode: "deep",
      });
      setAnswer(raw);
    } catch (error) {
      toast.error("Copilot request failed", { description: String(error) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <>
      <button
        className="btn"
        onClick={handleOpen}
        title={label}
        aria-label={label}
        style={{
          position: "fixed",
          right: 24,
          bottom: 24,
          zIndex: 100,
          borderRadius: "50%",
          width: 44,
          height: 44,
          padding: 0,
          background: "var(--accent)",
          color: "var(--elevated)",
          boxShadow: "0 10px 30px rgba(0,0,0,0.18)",
        }}
      >
        <I.Brain width={18} height={18} />
      </button>
      <Drawer open={open} onClose={() => setOpen(false)} title={label} width={520}>
        <div className="stack stack-md">
          <textarea
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
            rows={5}
            style={{
              width: "100%",
              border: "1px solid var(--line)",
              borderRadius: 10,
              background: "var(--surface-2)",
              color: "var(--ink)",
              padding: 12,
              resize: "vertical",
              font: "inherit",
              boxSizing: "border-box",
            }}
          />
          <div className="row row-sm" style={{ justifyContent: "flex-end" }}>
            <Button variant="ghost" onClick={() => setQuestion(prompt)}>
              Reset
            </Button>
            <Button variant="primary" onClick={() => void handleAsk()} loading={loading} disabled={!question.trim()}>
              <I.Send width={14} height={14} />
              Ask Copilot
            </Button>
          </div>
          {answer && (
            <div className="card" style={{ padding: 16 }}>
              <AgentResponseRenderer answer={answer} />
            </div>
          )}
        </div>
      </Drawer>
    </>
  );
}
