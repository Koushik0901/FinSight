import { z } from "zod";

export const FINANCE_ARTIFACT_SCHEMA_VERSION = 1;
export const FINANCE_ARTIFACT_MAX_BYTES = 24_000;

// Bounds applied *inside* the payload, on top of the overall byte cap, so a
// structurally-valid-but-pathological artifact (e.g. 100k table rows that each
// stay small) is still rejected before it reaches a renderer.
const MAX_TABLE_ROWS = 200;
const MAX_TABLE_COLS = 24;
const MAX_METRICS = 50;
const MAX_CHART_POINTS = 200;
const MAX_TEXT = 20_000;
const MAX_LABEL = 400;

const shortString = z.string().max(MAX_LABEL);

/// Discriminated union mirroring the Rust `AgentResponseBlock` — the only shape
/// the backend ever puts inside a `FinSightResponseBlock` artifact. Every branch
/// is bounded so an oversized or malformed block is rejected, not rendered.
export const CopilotResponseBlockSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("markdown"), markdown: z.string().max(MAX_TEXT) }),
  z.object({
    kind: z.literal("table"),
    title: shortString.nullable(),
    columns: z.array(shortString).max(MAX_TABLE_COLS),
    rows: z.array(z.array(shortString).max(MAX_TABLE_COLS)).max(MAX_TABLE_ROWS),
  }),
  z.object({
    kind: z.literal("barChart"),
    title: shortString.nullable(),
    seriesLabel: shortString.nullable(),
    data: z.array(z.object({ label: shortString, value: z.number().finite() })).max(MAX_CHART_POINTS),
  }),
  z.object({
    kind: z.literal("lineChart"),
    title: shortString.nullable(),
    seriesLabel: shortString.nullable(),
    data: z.array(z.object({ label: shortString, value: z.number().finite() })).max(MAX_CHART_POINTS),
  }),
  z.object({
    kind: z.literal("metricGrid"),
    metrics: z
      .array(
        z.object({
          label: shortString,
          value: shortString,
          detail: shortString.nullable(),
          tone: shortString.nullable(),
        }),
      )
      .max(MAX_METRICS),
  }),
  z.object({
    kind: z.literal("callout"),
    tone: shortString,
    title: shortString.nullable(),
    body: z.string().max(MAX_TEXT),
  }),
  z.object({
    kind: z.literal("transactionTable"),
    count: z.number().int().nonnegative(),
    totalCents: z.number().int(),
    rows: z
      .array(
        z.object({
          date: shortString,
          merchant: shortString,
          categoryKey: shortString,
          amountCents: z.number().int(),
          flag: shortString.nullable(),
        }),
      )
      .min(1)
      .max(MAX_TABLE_ROWS),
    more: z.number().int().nonnegative(),
  }),
  z.object({
    kind: z.literal("affordabilityVerdict"),
    canAfford: z.boolean(),
    headline: shortString,
    sub: shortString,
    caveat: shortString.nullable(),
    fundingSource: z.object({ label: shortString, detail: shortString }).nullable(),
  }),
  z.object({
    kind: z.literal("categoryBreakdown"),
    periodLabel: shortString,
    rows: z
      .array(z.object({ categoryKey: shortString, amountCents: z.number().int(), isFixed: z.boolean(), isLever: z.boolean() }))
      .min(1)
      .max(30),
  }),
  z.object({
    kind: z.literal("allocationSplit"),
    totalCents: z.number().int().positive(),
    segments: z
      .array(z.object({ label: shortString, amountCents: z.number().int().nonnegative(), rationale: shortString, categoryKey: shortString }))
      .min(1)
      .max(12),
  }),
  z.object({
    kind: z.literal("rankedOptions"),
    title: shortString,
    options: z
      .array(z.object({ rankTone: z.enum(["primary", "neutral", "muted"]), label: shortString, detail: shortString, rationale: shortString }))
      .min(1)
      .max(10),
  }),
]);

/// Strict per-component prop schemas. A component is only allowlisted if it has a
/// schema here; anything else is rejected as an unknown component. Keep this in
/// lockstep with what `renderers.tsx` can actually render.
export const COMPONENT_PROP_SCHEMAS = {
  FinSightResponseBlock: z.object({ block: CopilotResponseBlockSchema }),
} as const;

export type FinanceArtifactComponent = keyof typeof COMPONENT_PROP_SCHEMAS;

export const FinanceArtifactComponentSchema = z.enum(
  Object.keys(COMPONENT_PROP_SCHEMAS) as [FinanceArtifactComponent, ...FinanceArtifactComponent[]],
);

const FinanceArtifactEnvelopeBaseSchema = z.object({
  schemaVersion: z.literal(FINANCE_ARTIFACT_SCHEMA_VERSION),
  kind: z.literal("artifact"),
  component: FinanceArtifactComponentSchema,
  props: z.record(z.string(), z.unknown()),
  sourceToolName: z.string().nullable(),
  artifactId: z.string().min(1).max(MAX_LABEL),
  createdAt: z.string().min(1).max(MAX_LABEL),
});

export type FinanceArtifactEnvelope = z.infer<typeof FinanceArtifactEnvelopeBaseSchema>;

/// Full validation: base envelope shape + the component-specific prop schema.
/// Returns the validated envelope or null. Never throws.
function validateEnvelope(candidate: unknown): FinanceArtifactEnvelope | null {
  const base = FinanceArtifactEnvelopeBaseSchema.safeParse(candidate);
  if (!base.success) return null;
  const propSchema = COMPONENT_PROP_SCHEMAS[base.data.component];
  const props = propSchema.safeParse(base.data.props);
  if (!props.success) return null;
  return base.data;
}

export function byteLength(value: string) {
  return new TextEncoder().encode(value).byteLength;
}

export function parseFinanceArtifactEnvelope(payload: string): FinanceArtifactEnvelope | null {
  if (byteLength(payload) > FINANCE_ARTIFACT_MAX_BYTES) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(payload);
  } catch {
    return null;
  }
  return validateEnvelope(parsed);
}

export function serializeFinanceArtifactEnvelope(envelope: FinanceArtifactEnvelope): string | null {
  const validated = validateEnvelope(envelope);
  if (!validated) return null;
  const payload = JSON.stringify(validated);
  return byteLength(payload) <= FINANCE_ARTIFACT_MAX_BYTES ? payload : null;
}
