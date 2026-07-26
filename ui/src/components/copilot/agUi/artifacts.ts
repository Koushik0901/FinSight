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
/// Long-form counterpart of `requiredString`, for the MAX_TEXT-capped prose
/// fields (`markdown`, a callout `body`) that Rust likewise rejects when blank.
const requiredText = z.string().max(MAX_TEXT).refine((s) => s.trim().length > 0, {
  message: "must not be blank",
});

/// Discriminated union mirroring the Rust `AgentResponseBlock` — the only shape
/// the backend ever puts inside a `FinSightResponseBlock` artifact. Every branch
/// is bounded so an oversized or malformed block is rejected, not rendered.
///
/// Not exported directly: `CopilotResponseBlockSchema` below wraps it with the
/// cross-field checks that `z.discriminatedUnion` cannot host (its members must
/// be plain objects, so a `.refine()` on a branch is a construction error).
const CopilotResponseBlockUnion = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("markdown"), markdown: requiredText }),
  // `.min(1)` on columns mirrors Rust's `!columns.is_empty()`. The row-width
  // check (Rust's `row.len() == columns.len()`) can't live here because
  // `z.discriminatedUnion` only accepts plain object members — it's applied to
  // the whole union below. The `.max()` bounds stay at the ARTIFACT_MAX_* values
  // they mirror, deliberately looser than Rust's tighter semantic caps (8 cols /
  // 50 rows), which are enforced server-side before anything is emitted.
  z.object({
    kind: z.literal("table"),
    title: shortString.nullable(),
    columns: z.array(shortString).min(1).max(MAX_TABLE_COLS),
    rows: z.array(z.array(shortString).max(MAX_TABLE_COLS)).max(MAX_TABLE_ROWS),
  }),
  z.object({
    kind: z.literal("barChart"),
    title: shortString.nullable(),
    seriesLabel: shortString.nullable(),
    data: z.array(z.object({ label: shortString, value: z.number().finite() })).min(1).max(MAX_CHART_POINTS),
  }),
  z.object({
    kind: z.literal("lineChart"),
    title: shortString.nullable(),
    seriesLabel: shortString.nullable(),
    data: z.array(z.object({ label: shortString, value: z.number().finite() })).min(1).max(MAX_CHART_POINTS),
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
      .min(1)
      .max(MAX_METRICS),
  }),
  z.object({
    kind: z.literal("callout"),
    tone: shortString,
    title: shortString.nullable(),
    body: requiredText,
  }),
  z.object({
    kind: z.literal("transactionTable"),
    count: z.number().int().nonnegative(),
    totalCents: z.number().int(),
    rows: z
      .array(
        z.object({
          date: shortString,
          merchant: requiredString,
          categoryKey: requiredString,
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
    headline: requiredString,
    sub: requiredString,
    caveat: shortString.nullable(),
    fundingSource: z.object({ label: shortString, detail: shortString }).nullable(),
  }),
  z.object({
    kind: z.literal("categoryBreakdown"),
    periodLabel: requiredString,
    rows: z
      .array(z.object({ categoryKey: requiredString, amountCents: z.number().int(), isFixed: z.boolean(), isLever: z.boolean() }))
      .min(1)
      .max(30),
  }),
  z.object({
    kind: z.literal("allocationSplit"),
    totalCents: z.number().int().positive(),
    segments: z
      .array(z.object({ label: requiredString, amountCents: z.number().int().nonnegative(), rationale: shortString, categoryKey: shortString }))
      .min(1)
      .max(12),
  }),
  z.object({
    kind: z.literal("rankedOptions"),
    title: requiredString,
    options: z
      .array(z.object({ rankTone: z.enum(["primary", "neutral", "muted"]), label: requiredString, detail: shortString, rationale: shortString }))
      .min(1)
      .max(10),
  }),
  z.object({
    kind: z.literal("comparisonBars"),
    title: requiredString,
    current: z.object({ label: requiredString, amountCents: z.number().int().nonnegative() }),
    prior: z.object({ label: requiredString, amountCents: z.number().int().nonnegative() }),
  }),
  z.object({
    kind: z.literal("recategorizationPreview"),
    count: z.number().int().nonnegative(),
    rows: z.array(z.object({ merchant: shortString, categoryKey: shortString, confidence: z.number().min(0).max(1) })).min(1).max(20),
    more: z.number().int().nonnegative(),
    // `requiredString`, not `.min(1)`: Rust rejects a whitespace-only bundle id,
    // and a bundle id that does not resolve makes the approval action a dead end.
    bundleId: requiredString,
  }),
  z.object({
    kind: z.literal("spendingReview"),
    months: z
      .array(
        z.object({
          // `requiredString` even though Rust's PRE-hydration gate accepts a
          // blank label when `period` is set: this schema validates the payload
          // the server SENDS, and `prune_unhydrated_blocks` drops any month
          // hydration did not label. See the note above about the two contracts.
          label: requiredString,
          spentCents: z.number().int(),
          subtitle: shortString.nullable(),
          categories: z
            .array(z.object({ label: requiredString, amountCents: z.number().int(), tag: z.enum(["over", "fixed", "lever"]).nullable() }))
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
    // No `.min(1)`, matching the Rust pre-hydration gate: this kind is
    // server-rendered, so an empty `rows` is a well-formed block the server
    // simply has not filled. The server drops those before sending
    // (`renderable_after_hydration`), and `AccountsOverviewCard` renders
    // nothing for one — so "empty means show nothing" is the single shared
    // meaning on both sides. Rejecting it here instead would demote the block
    // to the generic tool row, which is noisier than silence.
    rows: z
      .array(z.object({ name: requiredString, subtitle: shortString.nullable(), typeLabel: requiredString, amountCents: z.number().int().nullable(), badge: shortString.nullable() }))
      .max(30)
      .optional(),
  }),
  z.object({
    kind: z.literal("spendTimeline"),
    title: shortString.nullable(),
    subtitle: shortString.nullable(),
    points: z
      .array(z.object({ label: requiredString, amountCents: z.number().int(), highlight: z.boolean().optional().default(false), annotation: shortString.nullable(), projected: z.boolean().optional().default(false) }))
      .min(2)
      .max(24),
  }),
  z.object({
    kind: z.literal("spendingDrivers"),
    title: requiredString,
    subtitle: shortString.nullable(),
    drivers: z
      .array(z.object({ label: requiredString, tag: z.enum(["planned", "trend", "prices", "anomaly", "creep", "mixed"]), amountDisplay: requiredString, note: shortString.nullable() }))
      .min(1)
      .max(8),
  }),
  z.object({
    kind: z.literal("watchList"),
    title: requiredString,
    items: z.array(z.object({ label: requiredString, detail: z.string().max(MAX_TEXT), amountDisplay: shortString.nullable() })).min(1).max(8),
  }),
  z.object({
    kind: z.literal("actionPlan"),
    title: shortString.nullable(),
    items: z.array(requiredString).min(1).max(8),
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
    // What the question is about, so the server knows what to enumerate.
    // Nullable because an unknown or absent type grounds to nothing and the
    // block falls back to free text — a degraded question still beats a
    // fabricated option list.
    referenceType: shortString.nullish(),
  }),
]);

/// The union plus the cross-field checks a discriminated-union branch cannot
/// carry. Mirrors Rust's `row.len() == columns.len()` for the generic `table`
/// kind: a ragged table renders cells under the wrong headers, and a number
/// shown against the wrong label is worse than no table at all.
export const CopilotResponseBlockSchema = CopilotResponseBlockUnion.superRefine((block, ctx) => {
  if (block.kind === "table" && block.rows.some((row) => row.length !== block.columns.length)) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      path: ["rows"],
      message: "every table row must have exactly one cell per column",
    });
  }
});

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
