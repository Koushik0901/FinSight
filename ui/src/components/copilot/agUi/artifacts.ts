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
// A clarification is a question the user must read and act on. Past a handful of
// choices a picker is worse than a text box, so the cap is a usability bound
// rather than a payload-size one.
const MAX_CLARIFICATION_OPTIONS = 8;

const shortString = z.string().max(MAX_LABEL);
/// Mirrors Rust's `!s.trim().is_empty()` checks — plain `.min(1)` would accept a
/// whitespace-only string that the Rust validator rejects, which is exactly the
/// kind of silent drift the parity corpus exists to catch.
const requiredString = shortString.refine((s) => s.trim().length > 0, {
  message: "must not be blank",
});

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
    query: z
      .object({
        merchant: shortString.nullable(),
        account: shortString.nullable(),
        startDate: shortString.nullable(),
        endDate: shortString.nullable(),
        minAmountCents: z.number().int().nullable(),
        direction: shortString.nullable(),
      })
      .nullish(),
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
  z.object({
    kind: z.literal("comparisonBars"),
    title: shortString,
    current: z.object({ label: shortString, amountCents: z.number().int().nonnegative() }),
    prior: z.object({ label: shortString, amountCents: z.number().int().nonnegative() }),
  }),
  z.object({
    kind: z.literal("recategorizationPreview"),
    count: z.number().int().nonnegative(),
    rows: z.array(z.object({ merchant: shortString, categoryKey: shortString, confidence: z.number().min(0).max(1) })).min(1).max(20),
    more: z.number().int().nonnegative(),
    bundleId: z.string().min(1).max(MAX_LABEL),
  }),
  z.object({
    kind: z.literal("spendingReview"),
    months: z
      .array(
        z.object({
          label: shortString,
          spentCents: z.number().int(),
          subtitle: shortString.nullable(),
          categories: z
            .array(z.object({ label: shortString, amountCents: z.number().int(), tag: z.enum(["over", "fixed", "lever"]).nullable() }))
            .max(10),
          summary: z.string().max(MAX_TEXT).nullable(),
          actions: z.array(shortString).max(6),
          period: shortString.nullish(),
        }),
      )
      .min(1)
      .max(6),
  }),
  z.object({
    kind: z.literal("accountsOverview"),
    title: shortString.nullable(),
    subtitle: shortString.nullable(),
    rows: z
      .array(z.object({ name: shortString, subtitle: shortString.nullable(), typeLabel: shortString, amountCents: z.number().int().nullable(), badge: shortString.nullable() }))
      .min(1)
      .max(30),
  }),
  z.object({
    kind: z.literal("spendTimeline"),
    title: shortString.nullable(),
    subtitle: shortString.nullable(),
    points: z
      .array(z.object({ label: shortString, amountCents: z.number().int(), highlight: z.boolean().optional().default(false), annotation: shortString.nullable(), projected: z.boolean().optional().default(false) }))
      .min(2)
      .max(24),
  }),
  z.object({
    kind: z.literal("spendingDrivers"),
    title: shortString,
    subtitle: shortString.nullable(),
    drivers: z
      .array(z.object({ label: shortString, tag: z.enum(["planned", "trend", "prices", "anomaly", "creep", "mixed"]), amountDisplay: shortString, note: shortString.nullable() }))
      .min(1)
      .max(8),
  }),
  z.object({
    kind: z.literal("watchList"),
    title: shortString,
    items: z.array(z.object({ label: shortString, detail: z.string().max(MAX_TEXT), amountDisplay: shortString.nullable() })).min(1).max(8),
  }),
  z.object({
    kind: z.literal("actionPlan"),
    title: shortString.nullable(),
    items: z.array(shortString).min(1).max(8),
  }),
  // A question the Copilot needs answered before it can continue. One shape
  // covers all three modes so the interaction reads as a single feature: no
  // `options` means free text only; with options, `multiSelect` picks single-
  // vs multi-choice. Options are SERVER-grounded from real data — the model
  // only chooses the question — so a hallucinated option can never become a
  // clickable answer.
  z.object({
    kind: z.literal("clarification"),
    clarificationId: requiredString,
    question: requiredString,
    multiSelect: z.boolean(),
    // No `.min(1)`: an empty array is the free-text mode, not a malformed
    // picker. Blank id/label are rejected — an unlabelled option is unclickable,
    // and an option whose id does not resolve cannot be answered.
    options: z
      .array(
        z.object({
          id: requiredString,
          label: requiredString,
          hint: shortString.nullable(),
        }),
      )
      .max(MAX_CLARIFICATION_OPTIONS),
    textPlaceholder: shortString.nullable(),
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
