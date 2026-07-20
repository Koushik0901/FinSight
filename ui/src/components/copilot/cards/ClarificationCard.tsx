import { useEffect, useState } from "react";
import { useThreadRuntime } from "@assistant-ui/react";
import * as I from "../../Icons";
import type { CopilotResponseBlock } from "../../../api/client";
import { useClarifications } from "../../../state/clarifications";

type Block = Extract<CopilotResponseBlock, { kind: "clarification" }>;

/**
 * A question the Copilot needs answered before it can continue.
 *
 * One component covers all three modes on purpose, so the interaction reads as
 * a single feature rather than two: no options means free text only; with
 * options, `multiSelect` picks single- vs multi-choice. Free text stays
 * available either way, so an option set that lacks the user's answer is never
 * a trap.
 *
 * Options are server-grounded from real data — the model only chooses the
 * question. That is what stops a hallucinated account becoming a clickable
 * answer that produces a confidently wrong result.
 *
 * Answering sends an ordinary user turn. Unlike the approval flow there is no
 * mutation to route: the answer is just the next message.
 */
export function ClarificationCard({ block }: { block: Block }) {
  const threadRuntime = useThreadRuntime();
  const resolved = useClarifications((s) => Boolean(s.resolved[block.clarificationId]));
  const requestBlock = useClarifications((s) => s.requestBlock);
  const release = useClarifications((s) => s.release);
  const clearIfPending = useClarifications((s) => s.clearIfPending);
  const [selected, setSelected] = useState<string[]>([]);
  const [text, setText] = useState("");

  // Re-registers on reload straight from the persisted block, which is why
  // recovery needs no extra conversation state.
  //
  // The cleanup matters as much as the registration: if this card goes away
  // for any reason other than being answered — switching conversation,
  // starting a new thread — the block has to lift with it. Otherwise the next
  // thread's composer is stuck on a question that is no longer on screen, with
  // no dismiss button to reach, because that button lives in this component.
  useEffect(() => {
    if (resolved) return;
    const id = block.clarificationId;
    requestBlock({ id, question: block.question });
    return () => clearIfPending(id);
  }, [resolved, requestBlock, clearIfPending, block.clarificationId, block.question]);

  const hasOptions = block.options.length > 0;

  const send = (answer: string) => {
    const trimmed = answer.trim();
    if (!trimmed) return;
    // Release first: the composer must come back even if the send throws,
    // otherwise a transient failure leaves the user with no way to type.
    release(block.clarificationId);
    threadRuntime.append({ role: "user", content: [{ type: "text", text: trimmed }] });
  };

  const toggle = (id: string) => {
    setSelected((prev) =>
      block.multiSelect
        ? prev.includes(id)
          ? prev.filter((x) => x !== id)
          : [...prev, id]
        : [id],
    );
  };

  const submitSelection = () => {
    // The hint rides along with the label. Account names have no uniqueness
    // constraint, so two options can read "Everyday" — sending the bare label
    // would hand the model the same string either way and it would be back to
    // guessing, which is the whole thing this block exists to stop.
    const chosen = block.options
      .filter((o) => selected.includes(o.id))
      .map((o) => (o.hint ? `${o.label} (${o.hint})` : o.label));
    if (chosen.length === 0) return;
    send(chosen.join(", "));
  };

  if (resolved) {
    // Keep the question visible so the thread still reads as a conversation,
    // but stop competing for the user's attention.
    return (
      <div className="cp-card cp-clarify is-resolved" data-testid="clarification-resolved">
        <div className="cp-clarify-q">{block.question}</div>
      </div>
    );
  }

  return (
    <div className="cp-card cp-clarify" data-testid="clarification">
      <div className="cp-clarify-head">
        <span className="cp-clarify-eyebrow">Needs an answer</span>
        <button
          type="button"
          className="cp-clarify-dismiss"
          aria-label="Dismiss question"
          onClick={() => release(block.clarificationId)}
        >
          <I.X width={12} height={12} />
        </button>
      </div>

      <div className="cp-clarify-q">{block.question}</div>

      {hasOptions && (
        <>
          <div className="cp-clarify-options" role={block.multiSelect ? "group" : "radiogroup"}>
            {block.options.map((option) => {
              const isOn = selected.includes(option.id);
              return (
                <button
                  key={option.id}
                  type="button"
                  role={block.multiSelect ? "checkbox" : "radio"}
                  aria-checked={isOn}
                  className={`cp-clarify-option${isOn ? " is-on" : ""}`}
                  onClick={() => toggle(option.id)}
                >
                  <span className="cp-clarify-option-label">{option.label}</span>
                  {option.hint && <span className="cp-clarify-option-hint">{option.hint}</span>}
                </button>
              );
            })}
          </div>
          <button
            type="button"
            className="cp-clarify-send"
            disabled={selected.length === 0}
            onClick={submitSelection}
          >
            {block.multiSelect && selected.length > 1 ? `Use these ${selected.length}` : "Use this"}
          </button>
        </>
      )}

      {/* Always present, options or not — the escape hatch for an answer the
          option set does not contain. */}
      <form
        className="cp-clarify-text"
        onSubmit={(e) => {
          e.preventDefault();
          send(text);
        }}
      >
        <input
          type="text"
          value={text}
          placeholder={block.textPlaceholder ?? (hasOptions ? "Or type an answer…" : "Type your answer…")}
          aria-label="Answer"
          onChange={(e) => setText(e.target.value)}
        />
        <button type="submit" disabled={text.trim() === ""} aria-label="Send answer">
          <I.ArrowUp width={14} height={14} />
        </button>
      </form>
    </div>
  );
}
