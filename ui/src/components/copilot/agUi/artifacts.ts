import { z } from "zod";

export const FINANCE_ARTIFACT_SCHEMA_VERSION = 1;
export const FINANCE_ARTIFACT_MAX_BYTES = 24_000;

export const FinanceArtifactComponentSchema = z.enum([
  "FinSightResponseBlock",
  "MetricGrid",
  "BudgetCard",
  "TransactionTable",
  "ScenarioChart",
  "ActionPlan",
  "DebtPayoffTable",
  "GoalProjection",
  "CashflowTimeline",
]);

export const FinanceArtifactEnvelopeSchema = z.object({
  schemaVersion: z.literal(FINANCE_ARTIFACT_SCHEMA_VERSION),
  kind: z.literal("artifact"),
  component: FinanceArtifactComponentSchema,
  props: z.record(z.string(), z.unknown()),
  sourceToolName: z.string().nullable(),
  artifactId: z.string().min(1),
  createdAt: z.string().min(1),
});

export type FinanceArtifactEnvelope = z.infer<typeof FinanceArtifactEnvelopeSchema>;

export function byteLength(value: string) {
  return new TextEncoder().encode(value).byteLength;
}

export function parseFinanceArtifactEnvelope(payload: string): FinanceArtifactEnvelope | null {
  if (byteLength(payload) > FINANCE_ARTIFACT_MAX_BYTES) return null;
  try {
    return FinanceArtifactEnvelopeSchema.parse(JSON.parse(payload));
  } catch {
    return null;
  }
}

export function serializeFinanceArtifactEnvelope(envelope: FinanceArtifactEnvelope): string | null {
  const validated = FinanceArtifactEnvelopeSchema.safeParse(envelope);
  if (!validated.success) return null;
  const payload = JSON.stringify(validated.data);
  return byteLength(payload) <= FINANCE_ARTIFACT_MAX_BYTES ? payload : null;
}
