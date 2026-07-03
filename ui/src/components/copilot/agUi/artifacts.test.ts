import { describe, expect, it } from "vitest";
import {
  FINANCE_ARTIFACT_MAX_BYTES,
  parseFinanceArtifactEnvelope,
  serializeFinanceArtifactEnvelope,
  type FinanceArtifactEnvelope,
} from "./artifacts";

const validEnvelope: FinanceArtifactEnvelope = {
  schemaVersion: 1,
  kind: "artifact",
  component: "FinSightResponseBlock",
  props: {
    block: {
      kind: "table",
      title: "Top spending",
      columns: ["Category", "Spent"],
      rows: [["Dining", "$8,370"]],
    },
  },
  sourceToolName: "get_top_spending_categories",
  artifactId: "artifact-1",
  createdAt: "2026-07-02T00:00:00.000Z",
};

describe("finance artifact envelope validation", () => {
  it("serializes and parses a valid FinSightResponseBlock envelope", () => {
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

  it("rejects unknown / no-longer-allowlisted components", () => {
    for (const component of ["ApprovalPanel", "BudgetCard", "ScenarioChart"]) {
      const payload = JSON.stringify({ ...validEnvelope, component });
      expect(parseFinanceArtifactEnvelope(payload)).toBeNull();
    }
  });

  it("rejects a FinSightResponseBlock whose props.block is malformed", () => {
    const payload = JSON.stringify({
      ...validEnvelope,
      props: { block: { kind: "table", columns: "not-an-array", rows: [] } },
    });
    expect(parseFinanceArtifactEnvelope(payload)).toBeNull();
  });

  it("rejects an unknown block kind", () => {
    const payload = JSON.stringify({
      ...validEnvelope,
      props: { block: { kind: "iframe", src: "http://evil" } },
    });
    expect(parseFinanceArtifactEnvelope(payload)).toBeNull();
  });

  it("rejects a table block that exceeds the row bound", () => {
    const rows = Array.from({ length: 500 }, () => ["x", "y"]);
    const payload = JSON.stringify({
      ...validEnvelope,
      props: { block: { kind: "table", title: null, columns: ["a", "b"], rows } },
    });
    expect(parseFinanceArtifactEnvelope(payload)).toBeNull();
  });

  it("validates a bounded TransactionTable stub payload", () => {
    const envelope: FinanceArtifactEnvelope = {
      ...validEnvelope,
      component: "TransactionTable",
      props: { title: "Recent", rows: [["Jul 1", "Tim Hortons", "-$9.43"]] },
    };
    const payload = serializeFinanceArtifactEnvelope(envelope);
    expect(payload).toBeTypeOf("string");
    expect(parseFinanceArtifactEnvelope(payload ?? "")).toEqual(envelope);
  });
});
