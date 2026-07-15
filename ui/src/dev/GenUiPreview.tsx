/**
 * DEV-ONLY gallery of the Copilot generative-UI finance blocks.
 *
 * Renders every `FinSightResponseBlock` kind through the SAME dispatcher the
 * live Copilot uses, with hand-written sample payloads — a visual check and a
 * regression aid for the card library. Gated behind `import.meta.env.DEV` in
 * App.tsx, so it never ships in a production build. Not a product surface.
 */
import type { CopilotResponseBlock } from "../api/client";
import { FinSightResponseBlock } from "../components/copilot/renderers";
import "../styles/copilot-shell.css";

const SAMPLES: { title: string; block: CopilotResponseBlock }[] = [
  {
    title: "spendingReview",
    block: {
      kind: "spendingReview",
      months: [
        {
          label: "May 2026",
          spentCents: 408600,
          subtitle: "8 of 10 envelopes under",
          categories: [
            { label: "Housing", amountCents: 185000, tag: "fixed" },
            { label: "Groceries", amountCents: 63200, tag: null },
            { label: "Dining", amountCents: 41200, tag: "over" },
            { label: "Utilities", amountCents: 30800, tag: null },
            { label: "Shopping", amountCents: 28600, tag: "lever" },
          ],
          summary:
            "A steady month. Spending landed at $4,086 and eight of ten envelopes finished with room to spare. PG&E came in at $220 — 2.1× your twelve-month average — and Dining crossed its $400 cap by $12.",
          actions: [
            "Glance at the PG&E bill — likely a billing-cycle overlap; worth a dispute if not",
            "Sweep the unused $168 in Groceries into the House Fund",
            "Hold the $400 Dining cap — May missed it by only $12",
          ],
        },
        {
          label: "June 2026",
          spentCents: 474800,
          subtitle: "+16% vs May",
          categories: [
            { label: "Housing", amountCents: 185000, tag: "fixed" },
            { label: "Groceries", amountCents: 71800, tag: null },
            { label: "Dining", amountCents: 48200, tag: "over" },
            { label: "Shopping", amountCents: 35400, tag: "lever" },
            { label: "Utilities", amountCents: 34200, tag: null },
          ],
          summary: "The hot month — Dining hit a record $482, driven almost entirely by Saturday evenings.",
          actions: ["Set a $420 Dining sub-cap for Saturdays", "Review the three largest Shopping orders"],
        },
      ],
    },
  },
  {
    title: "accountsOverview",
    block: {
      kind: "accountsOverview",
      title: "7 accounts",
      subtitle: "$137,515 tracked · 1 missing a balance",
      rows: [
        { name: "Joint Checking", subtitle: "Mercury ····4421", typeLabel: "Checking", amountCents: 1482042, badge: null },
        { name: "House Fund", subtitle: "Wealthfront ····9087", typeLabel: "Savings", amountCents: 2864000, badge: null },
        { name: "Amex Gold", subtitle: "Amex ····1006", typeLabel: "Credit", amountCents: -241800, badge: null },
        { name: "Retirement", subtitle: "Fidelity ····0814", typeLabel: "Investment", amountCents: 8642000, badge: null },
        { name: "Vanguard Brokerage", subtitle: "manual · added Mar 2026", typeLabel: "Investment", amountCents: null, badge: "needs a balance set" },
      ],
    },
  },
  {
    title: "spendTimeline",
    block: {
      kind: "spendTimeline",
      title: "Monthly spend · Jan–Jul 2026",
      subtitle: "last 3 months highlighted · July projected",
      points: [
        { label: "Jan", amountCents: 360000, highlight: false, annotation: null, projected: false },
        { label: "Feb", amountCents: 370000, highlight: false, annotation: null, projected: false },
        { label: "Mar", amountCents: 430000, highlight: false, annotation: null, projected: false },
        { label: "Apr", amountCents: 570000, highlight: false, annotation: "LISBON", projected: false },
        { label: "May", amountCents: 408600, highlight: true, annotation: null, projected: false },
        { label: "Jun", amountCents: 474800, highlight: true, annotation: null, projected: false },
        { label: "Jul", amountCents: 440000, highlight: true, annotation: null, projected: true },
      ],
    },
  },
  {
    title: "spendingDrivers",
    block: {
      kind: "spendingDrivers",
      title: "What's actually driving the +$728/mo",
      subtitle: "vs your Jan–Feb baseline",
      drivers: [
        { label: "Travel", tag: "planned", amountDisplay: "+$213/mo", note: "Italy flight deposits — funded by the Italy goal, not a leak" },
        { label: "Dining", tag: "trend", amountDisplay: "+$127/mo", note: "Up 42% on your winter pace, concentrated on Saturday evenings" },
        { label: "Groceries", tag: "prices", amountDisplay: "+$88/mo", note: "Same stores, same cadence — unit prices are up" },
        { label: "Utilities", tag: "anomaly", amountDisplay: "+$49/mo", note: "PG&E ran 2.1× average in May; summer AC in June" },
        { label: "Subscriptions", tag: "creep", amountDisplay: "+$30/mo", note: "$161 → $195 in five months · Adobe +$3, Disney+ unused" },
        { label: "Everything else", tag: "mixed", amountDisplay: "+$221/mo", note: "Shopping, gifts, health — small drifts, no single culprit" },
      ],
    },
  },
  {
    title: "watchList",
    block: {
      kind: "watchList",
      title: "Watch out for these",
      items: [
        { label: "The Amex balance", detail: "$2,418 revolving at 24.9% — interest quietly compounds the problem", amountDisplay: "−$50/mo" },
        { label: "MasterClass trial converting", detail: "Free trial flips to $180/yr on the 26th unless cancelled", amountDisplay: "$180/yr" },
      ],
    },
  },
  {
    title: "actionPlan",
    block: {
      kind: "actionPlan",
      title: "Action plan",
      items: [
        "Sweep the unused $168 into the House Fund",
        "Cancel the MasterClass trial before the 26th",
        "Set the Vanguard balance so net worth is complete",
      ],
    },
  },
];

export default function GenUiPreview() {
  return (
    <div className="copilot-screen" style={{ padding: 24, maxWidth: 760, margin: "0 auto" }}>
      <h1 style={{ color: "var(--ink)", marginBottom: 4 }}>Copilot generative-UI blocks</h1>
      <p className="muted" style={{ marginBottom: 24 }}>
        DEV preview — every block rendered through <code>FinSightResponseBlock</code>.
      </p>
      <div className="copilot-bubble-asst" style={{ display: "flex", flexDirection: "column", gap: 28 }}>
        {SAMPLES.map(({ title, block }) => (
          <section key={title}>
            <p className="eyebrow" style={{ marginBottom: 10 }}>{title}</p>
            <FinSightResponseBlock block={block} isRunning={false} />
          </section>
        ))}
      </div>
    </div>
  );
}
