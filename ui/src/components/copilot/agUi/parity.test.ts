import { describe, it, expect } from "vitest";
import { CopilotResponseBlockSchema } from "./artifacts";
import cases from "./response_blocks.fixture.json";

/**
 * Frontend half of the Rust<->Zod parity corpus. Imports the SAME fixture file
 * the Rust `rust_verdicts_match_the_parity_corpus` test reads, and asserts Zod's
 * accept/reject verdict matches `expectValid` for every case. A mismatch here (or
 * on the Rust side) means the two hand-maintained validations have drifted — the
 * failure names the exact block kind so the field-level bound can be reconciled.
 *
 * Note for the server-rendered kinds (`accountsOverview`, `spendingReview`): the
 * Rust half checks the PRE-hydration request gate while this half checks the
 * payload the server actually sends. Those agree only because "empty" carries
 * one meaning on both sides — the server drops an unfilled block and the card
 * renders nothing for one. A single `expectValid` boolean cannot express "valid
 * as a request, invalid as a rendering", so cases where the two readings would
 * differ belong in a Rust-only test, not this corpus.
 */
const corpus = cases as { expectValid: boolean; block: { kind: string } }[];

describe("Rust/Zod parity corpus", () => {
  it("loads the shared fixture", () => {
    expect(corpus.length).toBeGreaterThan(0);
  });

  it.each(corpus)("Zod verdict matches for kind $block.kind (expectValid=$expectValid)", ({ expectValid, block }) => {
    expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(expectValid);
  });
});
