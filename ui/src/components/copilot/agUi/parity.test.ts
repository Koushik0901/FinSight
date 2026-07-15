import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { CopilotResponseBlockSchema } from "./artifacts";

/**
 * Frontend half of the Rust<->Zod parity corpus. Loads the SAME fixture file the
 * Rust `rust_verdicts_match_the_parity_corpus` test uses, and asserts Zod's
 * accept/reject verdict matches `expectValid` for every case. A mismatch here (or
 * on the Rust side) means the two hand-maintained validations have drifted — the
 * failure names the exact block kind so the field-level bound can be reconciled.
 */
const FIXTURE = resolve(
  __dirname,
  "../../../../../crates/finsight-app/tests/fixtures/response_blocks.json",
);

const cases = JSON.parse(readFileSync(FIXTURE, "utf8")) as {
  expectValid: boolean;
  block: { kind: string };
}[];

describe("Rust/Zod parity corpus", () => {
  it("loads the shared fixture", () => {
    expect(cases.length).toBeGreaterThan(0);
  });

  it.each(cases)("Zod verdict matches for kind $block.kind (expectValid=$expectValid)", ({ expectValid, block }) => {
    expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(expectValid);
  });
});
