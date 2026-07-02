import { AbstractAgent, EventType, type BaseEvent, type RunAgentInput } from "@ag-ui/client";
import { Observable } from "rxjs";

const STREAM_DELAY_MS = 180;

function makeId(prefix: string) {
  const uuid = globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2);
  return `${prefix}-${uuid}`;
}

export class TauriAgUiSpikeAgent extends AbstractAgent {
  constructor() {
    super({
      agentId: "finsight-tauri-ag-ui-spike",
      description: "FinSight Phase 0 AG-UI custom-agent compatibility spike",
      threadId: "finsight-ag-ui-spike-thread",
    });
  }

  override run(input: RunAgentInput): Observable<BaseEvent> {
    return new Observable<BaseEvent>((subscriber) => {
      const threadId = input.threadId;
      const runId = input.runId;
      const assistantMessageId = makeId("agui-spike-assistant");
      const timers: ReturnType<typeof setTimeout>[] = [];

      const emit = (event: BaseEvent) => {
        if (!subscriber.closed) {
          subscriber.next(event);
        }
      };

      emit({
        type: EventType.RUN_STARTED,
        threadId,
        runId,
        input,
      });
      emit({
        type: EventType.TEXT_MESSAGE_START,
        messageId: assistantMessageId,
        role: "assistant",
      });

      const chunks = [
        "AG-UI Phase 0 spike is streaming from a custom `AbstractAgent`. ",
        "This path is isolated behind `copilot.agUiRuntime` and does not touch the existing FinSight Copilot runtime. ",
        "If this renders progressively in Tauri, Phase 1 can safely map real Rust stream frames next.",
      ];

      chunks.forEach((delta, index) => {
        timers.push(
          setTimeout(() => {
            emit({
              type: EventType.TEXT_MESSAGE_CONTENT,
              messageId: assistantMessageId,
              delta,
            });

            if (index === chunks.length - 1) {
              emit({
                type: EventType.TEXT_MESSAGE_END,
                messageId: assistantMessageId,
              });
              emit({
                type: EventType.RUN_FINISHED,
                threadId,
                runId,
                outcome: { type: "success" },
              });
              subscriber.complete();
            }
          }, STREAM_DELAY_MS * (index + 1)),
        );
      });

      return () => {
        timers.forEach((timer) => clearTimeout(timer));
      };
    });
  }
}
