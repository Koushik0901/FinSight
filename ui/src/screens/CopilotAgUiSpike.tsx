import { useMemo } from "react";
import {
  AssistantRuntimeProvider,
  ComposerPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
} from "@assistant-ui/react";
import { useAgUiRuntime } from "@assistant-ui/react-ag-ui";
import * as I from "../components/Icons";
import {
  copilotAgUiRuntimeFlag,
  isCopilotAgUiRuntimeEnabled,
} from "../components/copilot/agUi/featureFlag";
import { TauriAgUiSpikeAgent } from "../components/copilot/agUi/TauriAgUiSpikeAgent";

function Gate() {
  return (
    <section className="card" style={{ maxWidth: 760, margin: "48px auto", padding: 24 }}>
      <p className="eyebrow">Copilot AG-UI Phase 0</p>
      <h1 style={{ marginTop: 8 }}>AG-UI spike is disabled</h1>
      <p className="muted">
        The default-off flag is working. Open this route with{" "}
        <code>?{copilotAgUiRuntimeFlag.queryParam}=1</code> or set{" "}
        <code>{copilotAgUiRuntimeFlag.storageKey}</code> to <code>true</code> in local storage.
      </p>
    </section>
  );
}

function AssistantMessage() {
  return (
    <MessagePrimitive.Root className="copilot-msg-asst">
      <div className="copilot-avatar">
        <I.Brain width={14} height={14} style={{ color: "var(--accent)" }} />
      </div>
      <div className="copilot-bubble-asst">
        <MessagePrimitive.Parts />
      </div>
    </MessagePrimitive.Root>
  );
}

function UserMessage() {
  return (
    <MessagePrimitive.Root className="copilot-msg-user">
      <div className="copilot-bubble-user">
        <MessagePrimitive.Parts />
      </div>
    </MessagePrimitive.Root>
  );
}

function Composer() {
  return (
    <ComposerPrimitive.Root className="copilot-composer">
      <ComposerPrimitive.Input
        className="copilot-composer-input"
        placeholder="Send any message to verify AG-UI custom-agent streaming..."
      />
      <ComposerPrimitive.Send className="copilot-send-btn" aria-label="Send AG-UI spike message">
        <I.ArrowUp width={18} height={18} />
      </ComposerPrimitive.Send>
    </ComposerPrimitive.Root>
  );
}

function SpikeThread() {
  return (
    <section className="copilot-shell" data-testid="ag-ui-spike-surface">
      <div className="copilot-main">
        <header className="copilot-topbar">
          <div>
            <p className="copilot-kicker">
              <span className="copilot-live-dot" /> Copilot · AG-UI Phase 0
            </p>
            <h1>Custom-agent streaming spike</h1>
          </div>
          <span className="chip">feature flag: off by default</span>
        </header>

        <ThreadPrimitive.Root className="copilot-thread">
          <ThreadPrimitive.Viewport className="copilot-viewport copilot-scrollbar">
            <ThreadPrimitive.Empty>
              <div className="copilot-empty">
                <p className="eyebrow">Isolated runtime spike</p>
                <h2>Test AG-UI without touching the current Copilot</h2>
                <p className="muted">
                  Send a message. The response should appear progressively from a local custom{" "}
                  <code>AbstractAgent</code>.
                </p>
              </div>
            </ThreadPrimitive.Empty>
            <ThreadPrimitive.Messages
              components={{
                UserMessage,
                AssistantMessage,
              }}
            />
            <ThreadPrimitive.ViewportFooter className="copilot-viewport-footer">
              <div className="copilot-composer-wrap">
                <Composer />
              </div>
            </ThreadPrimitive.ViewportFooter>
          </ThreadPrimitive.Viewport>
        </ThreadPrimitive.Root>
      </div>
    </section>
  );
}

export default function CopilotAgUiSpike() {
  const enabled = isCopilotAgUiRuntimeEnabled();
  const agent = useMemo(() => new TauriAgUiSpikeAgent(), []);
  const runtime = useAgUiRuntime({
    agent,
    showThinking: true,
  });

  if (!enabled) {
    return <Gate />;
  }

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <SpikeThread />
    </AssistantRuntimeProvider>
  );
}
