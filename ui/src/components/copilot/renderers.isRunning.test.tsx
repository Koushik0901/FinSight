import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// The bug this test guards against: `render_finance_artifact`'s own tool-call
// part completes as soon as its result arrives, which happens BEFORE prose
// streaming starts (the backend emits response blocks, then streams text
// word-by-word). So the tool-call part's own `status` is "complete" for
// nearly the entire time the message is still streaming — using it as the
// "is this message still streaming" signal defeats the whole point of
// deferring the ComparisonBars chart mount until streaming really finishes.
// CopilotToolCard must instead read the real message-level running state via
// useMessage(), not the tool-call part's own status.
vi.mock("@assistant-ui/react", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@assistant-ui/react")>();
  return {
    ...actual,
    useMessage: () => ({ status: { type: "running" } }),
  };
});

import { CopilotToolCard } from "./renderers";

describe("CopilotToolCard isRunning derivation", () => {
  it("treats the artifact as still-streaming based on the message's status, not the tool-call part's own (already-complete) status", () => {
    const envelope = {
      schemaVersion: 1,
      kind: "artifact",
      component: "FinSightResponseBlock",
      props: {
        block: {
          kind: "comparisonBars",
          title: "Dining",
          current: { label: "May", amountCents: 100 },
          prior: { label: "Apr", amountCents: 80 },
        },
      },
      sourceToolName: null,
      artifactId: "block-0",
      createdAt: new Date().toISOString(),
    };

    render(
      <CopilotToolCard
        toolName="render_finance_artifact"
        args={{}}
        result={JSON.stringify(envelope)}
        isError={false}
        // The tool-call part itself is "complete" — its result already
        // arrived — even though the overall message (mocked above) is still
        // "running". This is the exact real-world state during streaming.
        status={{ type: "complete" }}
      />
    );

    // If isRunning were (incorrectly) derived from the tool-call part's own
    // "complete" status, the chart would mount immediately and this
    // placeholder would be absent. Since it's derived from the message's
    // real "running" status instead, the placeholder must show.
    expect(screen.getByText("Dining")).toBeInTheDocument();
    expect(screen.getByText(/Preparing comparison/i)).toBeInTheDocument();
  });
});
