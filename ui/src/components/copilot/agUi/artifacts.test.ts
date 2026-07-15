import { describe, expect, it } from "vitest";
import {
  CopilotResponseBlockSchema,
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

  it("validates a bounded transactionTable block payload", () => {
    const envelope: FinanceArtifactEnvelope = {
      ...validEnvelope,
      props: {
        block: {
          kind: "transactionTable",
          count: 42,
          totalCents: 1_193_000,
          rows: [
            {
              date: "2026-05-03",
              merchant: "Bay Property · Rent",
              categoryKey: "Housing",
              amountCents: 185_000,
              flag: null,
            },
          ],
          more: 32,
        },
      },
    };
    const payload = serializeFinanceArtifactEnvelope(envelope);
    expect(payload).toBeTypeOf("string");
    expect(parseFinanceArtifactEnvelope(payload ?? "")).toEqual(envelope);
  });

  it("rejects a transactionTable block with zero rows", () => {
    const payload = JSON.stringify({
      ...validEnvelope,
      props: {
        block: {
          kind: "transactionTable",
          count: 0,
          totalCents: 0,
          rows: [],
          more: 0,
        },
      },
    });
    expect(parseFinanceArtifactEnvelope(payload)).toBeNull();
  });
});

describe("spendingReview block schema", () => {
  it("accepts a valid block", () => {
    const block = {
      kind: "spendingReview",
      months: [
        {
          label: "May 2026",
          spentCents: 408600,
          subtitle: "8 of 10 envelopes under",
          categories: [{ label: "Housing", amountCents: 185000, tag: "fixed" }],
          summary: "Steady.",
          actions: ["Do X"],
        },
      ],
    };
    expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(true);
  });

  it("rejects an unknown category tag", () => {
    const block = {
      kind: "spendingReview",
      months: [
        {
          label: "May",
          spentCents: 1,
          subtitle: null,
          categories: [{ label: "X", amountCents: 1, tag: "bogus" }],
          summary: null,
          actions: [],
        },
      ],
    };
    expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(false);
  });

  it("rejects more than 6 months", () => {
    const month = { label: "M", spentCents: 1, subtitle: null, categories: [], summary: null, actions: [] };
    const block = { kind: "spendingReview", months: Array(7).fill(month) };
    expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(false);
  });
});
