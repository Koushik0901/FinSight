import { describe, expect, it } from "vitest";
import {
  FINANCE_ARTIFACT_MAX_BYTES,
  parseFinanceArtifactEnvelope,
  serializeFinanceArtifactEnvelope,
} from "./artifacts";

const validEnvelope = {
  schemaVersion: 1,
  kind: "artifact",
  component: "BudgetCard",
  props: { title: "July budget", amountCents: 120000 },
  sourceToolName: "get_budgets",
  artifactId: "artifact-1",
  createdAt: "2026-07-02T00:00:00.000Z",
} as const;

describe("finance artifact envelope validation", () => {
  it("serializes and parses a valid allowlisted artifact envelope", () => {
    const payload = serializeFinanceArtifactEnvelope(validEnvelope);

    expect(payload).toBeTypeOf("string");
    expect(parseFinanceArtifactEnvelope(payload ?? "")).toEqual(validEnvelope);
  });

  it("rejects unsupported schema versions", () => {
    const payload = JSON.stringify({ ...validEnvelope, schemaVersion: 2 });

    expect(parseFinanceArtifactEnvelope(payload)).toBeNull();
  });

  it("rejects oversized payloads before treating them as artifacts", () => {
    const payload = `${"{".padEnd(FINANCE_ARTIFACT_MAX_BYTES + 1, "x")}`;

    expect(parseFinanceArtifactEnvelope(payload)).toBeNull();
  });

  it("rejects invalid JSON", () => {
    expect(parseFinanceArtifactEnvelope("{not-json")).toBeNull();
  });

  it("rejects unknown components", () => {
    const payload = JSON.stringify({ ...validEnvelope, component: "ApprovalPanel" });

    expect(parseFinanceArtifactEnvelope(payload)).toBeNull();
  });
});
