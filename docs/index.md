---
layout: page
title: FinSight — Your AI-powered financial copilot
description: A quiet way to understand, plan, and master your money. Local-first, encrypted, self-hosted.
head:
  - - meta
    - property: og:title
      content: FinSight — Your AI-powered financial copilot
  - - meta
    - property: og:description
      content: A quiet way to understand, plan, and master your money. Local-first, encrypted, self-hosted.
---

<HomeHero />
<script setup lang="ts">
import { withBase } from "vitepress"
</script>

<div class="home-section">

## Understand. Plan. Act.

<div class="home-lead">FinSight turns fragmented money data into calm, actionable guidance — without handing your ledger to a vendor.</div>

<FeatureGrid>
  <FeatureCard icon="search" title="Understand" description="See where your money goes, why it moves, and what patterns actually matter — categories, merchant history, recurring detection, and a daily review queue." link="/guide/overview" linkText="How it looks →" />
  <FeatureCard icon="lego" title="Plan" description="Envelopes, goals, scenarios and cash-flow forecasting — all deterministic finance math, then explained by the Copilot in plain language." link="/guide/budget" linkText="Budgets & goals →" />
  <FeatureCard icon="bolt" title="Act" description="The Copilot turns insights into next steps: plans, tool calls, and reviewable action bundles you approve — no silent automation." link="/copilot/overview" linkText="Meet the Copilot →" />
  <FeatureCard icon="recipe" title="Automate" description="Rules, recipes, and the categorizer handle the repetitive parts — pattern rules you can read, deterministic fallbacks when AI is off." link="/automation/overview" linkText="Rules & recipes →" />
  <FeatureCard icon="lock" title="Stay Private" description="One encrypted SQLCipher database per user, on hardware you control. Provider keys live in that database, not a global secret store." link="/getting-started/privacy" linkText="Privacy design →" />
  <FeatureCard icon="house" title="Self-Hosted" description="One Docker image, one /data volume. Tailscale, Caddy, or LAN — pick a recipe and keep your ledger at home." link="/getting-started/installation" linkText="Self-hosting →" />
</FeatureGrid>

</div>

<Philosophy />

<div class="home-section">

## Get started in minutes

<StepList :steps="[
  { title: 'Install FinSight', desc: 'docker compose up -d pulls the public image and creates your /data volume. No compile on the server.' },
  { title: 'Create your admin account', desc: 'First launch shows the setup wizard. Save the one-time recovery key — it is the only way back if a password is lost.' },
  { title: 'Add or import accounts', desc: 'Create manual accounts or connect SimpleFIN. Import years of CSV history; FinSight deduplicates on import.' },
  { title: 'Configure AI (optional)', desc: 'Use Ollama to keep inference local, or add an OpenAI-compatible or Anthropic provider when you choose. Copilot and auto-categorization are opt-in.' },
  { title: 'Start with Copilot', desc: 'Ask “What should I fix this month?” — FinSight grounds answers in your actual ledger, not generic advice.' },
]" />

<div class="home-cta-row">
  <a class="home-cta-primary" :href="withBase('/getting-started/introduction')">Read the getting-started guide</a>
  <a class="home-cta-secondary" :href="withBase('/help/self-hosting')">Self-hosting recipes →</a>
</div>

</div>

<div class="home-section muted">

### What FinSight is not

- Not a hosted SaaS that holds your data.
- Not a stock-picking or crypto-trading app.
- Not a noisy fintech dashboard. If you want piles of coins, confetti, and streaks, this is the wrong product.

FinSight is a quiet instrument — the kind you check in the morning, make one good decision, and close.

</div>

<style>
.home-section { max-width: 1152px; margin: 0 auto; padding: 36px 24px; }
.home-section.muted { background: var(--vp-c-bg-soft); border-top:1px solid var(--vp-c-divider); border-bottom:1px solid var(--vp-c-divider); }
.dark .home-section.muted { background: #0B0B0F; }
.home-section h2 { font-size: 28px; font-weight: 700; letter-spacing: -0.02em; line-height:1.15; margin:0 0 12px; color: var(--vp-c-text-1); }
.home-section h3 { font-size: 18px; margin:24px 0 8px; }
.home-lead { font-size: 15px; line-height:1.6; color: var(--vp-c-text-2); max-width: 64ch; margin:0 0 20px; }
.home-cta-row { display:flex; gap:12px; flex-wrap:wrap; margin-top: 28px; align-items:center; }
.home-cta-primary { display:inline-flex; align-items:center; height:40px; padding:0 20px; border-radius:999px; background: var(--vp-c-brand-1); color:#0A0F02; font-weight:620; font-size:14px; text-decoration:none; }
.dark .home-cta-primary { background:#C9F950; }
:root:not(.dark) .home-cta-primary { background:#0A0F02; color:#C9F950; }
.home-cta-secondary { font-size:14px; font-weight:550; color:var(--vp-c-text-2); text-decoration:none; padding:8px 4px; }
.home-cta-secondary:hover { color: var(--vp-c-brand-1); }
@media (max-width: 640px){ .home-section{ padding: 28px 16px; } }
</style>
