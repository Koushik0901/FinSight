<template>
  <section class="home-hero">
    <div class="home-hero-inner">
      <div class="home-hero-copy">
        <p class="home-kicker">
          <span class="kicker-dot" aria-hidden="true" />
          <Icon name="lock" style="width:12px;height:12px;opacity:0.7" />
          Local-first · Encrypted · Self-hosted
        </p>
        <h1 class="home-title">
          FinSight
          <span class="home-title-sub">Your AI-powered<br />financial copilot.</span>
        </h1>
        <p class="home-tagline">“A quiet way to understand, plan, and master your money.”</p>
        <div class="home-actions">
          <a class="btn-primary" :href="withBase('/getting-started/introduction')">Get Started</a>
          <a class="btn-secondary" href="https://github.com/Koushik0901/FinSight" target="_blank" rel="noopener">View on GitHub</a>
          <a class="btn-ghost" :href="withBase('/copilot/overview')">Explore the Copilot →</a>
        </div>
        <div class="home-privacy">
          <div class="home-privacy-label"><Icon name="lock" style="width:13px;height:13px" /> Your finances stay yours.</div>
          <p>Each user gets a separate SQLCipher database. Data leaves your machine only when you opt into a provider — or keep it local with Ollama.</p>
        </div>
      </div>
      <div class="home-hero-visual" aria-hidden="true">
        <div class="visual-card">
          <div class="visual-card-head">
            <span class="visual-dot" />
            <span class="visual-title">Today</span>
            <span class="visual-pill">Private</span>
          </div>
          <div class="visual-metric">
            <span class="visual-metric-label">Net worth</span>
            <span class="visual-metric-value">$84,320</span>
            <span class="visual-metric-sub">↑ 3.2% this month</span>
          </div>
          <div class="visual-bars" aria-hidden="true">
            <span class="bar" style="height: 28%"><i style="background: var(--c-housing)"/></span>
            <span class="bar" style="height: 46%"><i style="background: var(--c-groceries)"/></span>
            <span class="bar" style="height: 38%"><i style="background: var(--c-dining)"/></span>
            <span class="bar" style="height: 62%"><i style="background: var(--c-transport)"/></span>
            <span class="bar" style="height: 22%"><i style="background: var(--c-subs)"/></span>
            <span class="bar" style="height: 54%"><i style="background: var(--c-health)"/></span>
            <span class="bar" style="height: 34%"><i style="background: #A78BFA"/></span>
          </div>
          <div class="visual-foot">
            <span class="visual-chip">Needs 52%</span>
            <span class="visual-chip">Wants 21%</span>
            <span class="visual-chip accent">Savings 27%</span>
          </div>
          <div class="visual-copilot">
            <span class="vc-icon">✦</span>
            <span class="vc-text">“You’re on track to hit your emergency fund in 3 months.”</span>
          </div>
        </div>
        <div class="visual-card small">
          <div class="small-head">Budget · March</div>
          <div class="small-row"><span>Groceries</span><span><em>68%</em> $410 / $600</span></div>
          <div class="small-bar"><span style="width: 68%"/></div>
          <div class="small-row"><span>Dining</span><span><em>92%</em> $276 / $300</span></div>
          <div class="small-bar warn"><span style="width: 92%"/></div>
          <div class="small-row"><span>Savings</span><span><em>100%</em> $800 / $800</span></div>
          <div class="small-bar ok"><span style="width: 100%"/></div>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { withBase } from "vitepress";
</script>

<style scoped>
.home-hero {
  padding: 40px 0 32px;
  border-bottom: 1px solid var(--vp-c-divider);
}
.home-hero-inner {
  max-width: 1152px;
  margin: 0 auto;
  padding: 0 24px;
  display: grid;
  grid-template-columns: 1.05fr 0.95fr;
  gap: 48px;
  align-items: center;
}
@media (max-width: 960px) {
  .home-hero-inner { grid-template-columns: 1fr; gap: 32px; }
  .home-hero-visual { order: -1; }
}
.home-kicker {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-family: var(--fs-mono);
  font-size: 11px;
  letter-spacing: 0.09em;
  text-transform: uppercase;
  color: var(--vp-c-text-3);
  margin: 0 0 18px;
  font-weight: 600;
}
.kicker-dot {
  width: 6px; height: 6px; border-radius: 999px;
  background: var(--vp-c-brand-1);
  box-shadow: 0 0 0 4px var(--vp-c-brand-soft);
  display: inline-block;
}
.home-title {
  font-size: clamp(42px, 5vw, 56px);
  font-weight: 750;
  letter-spacing: -0.03em;
  line-height: 0.95;
  margin: 0;
  color: var(--vp-c-text-1);
}
.home-title-sub {
  display: block;
  font-size: clamp(26px, 3.2vw, 32px);
  font-weight: 500;
  letter-spacing: -0.02em;
  line-height: 1.15;
  margin-top: 12px;
  color: var(--vp-c-text-2);
}
.home-tagline {
  margin: 16px 0 0;
  font-size: 16px;
  color: var(--vp-c-text-3);
  font-style: italic;
  line-height: 1.5;
}
.home-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 28px;
  align-items: center;
}
.btn-primary {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 40px;
  padding: 0 20px;
  border-radius: 999px;
  background: var(--vp-c-brand-1);
  color: #0A0F02;
  font-weight: 620;
  font-size: 14px;
  text-decoration: none;
  border: 1px solid transparent;
  transition: filter 140ms, transform 140ms;
}
.btn-primary:hover { filter: brightness(1.05); transform: translateY(-1px); }
.btn-primary:active { transform: translateY(0) scale(0.98); }
.dark .btn-primary { background: #C9F950; color: #0A0F02; }
:root:not(.dark) .btn-primary { background: #0A0F02; color: #C9F950; border-color: #0A0F02; }
.btn-secondary {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 40px;
  padding: 0 18px;
  border-radius: 999px;
  background: transparent;
  border: 1px solid var(--vp-c-divider);
  color: var(--vp-c-text-1);
  font-weight: 550;
  font-size: 14px;
  text-decoration: none;
  transition: border-color 140ms, background 140ms;
}
.btn-secondary:hover { border-color: var(--vp-c-text-3); background: var(--vp-c-bg-soft); }
.btn-ghost {
  font-size: 14px;
  font-weight: 500;
  color: var(--vp-c-text-2);
  text-decoration: none;
  padding: 8px 4px;
}
.btn-ghost:hover { color: var(--vp-c-brand-1); }
.home-privacy {
  margin-top: 28px;
  padding: 16px 18px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  background: var(--vp-c-bg-soft);
  max-width: 520px;
}
.dark .home-privacy { background: #0F0F14; border-color: rgba(255,255,255,0.06); }
.home-privacy-label {
  font-size: 13px;
  font-weight: 650;
  color: var(--vp-c-text-1);
  margin-bottom: 6px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.home-privacy p {
  margin: 0;
  font-size: 13.5px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

/* Visual */
.home-hero-visual {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.visual-card {
  border: 1px solid var(--vp-c-divider);
  border-radius: 16px;
  background: var(--vp-c-bg-elv);
  padding: 18px;
  box-shadow: 0 1px 2px rgba(0,0,0,0.06), 0 12px 32px -16px rgba(0,0,0,0.12);
}
.dark .visual-card { background: #101015; border-color: rgba(255,255,255,0.06); box-shadow: 0 1px 0 rgba(255,255,255,0.04) inset, 0 16px 40px rgba(0,0,0,0.32); }
.visual-card.small { padding: 14px 16px; }
.visual-card-head {
  display: flex; align-items: center; gap: 8px; margin-bottom: 16px;
}
.visual-dot { width: 8px; height: 8px; border-radius: 999px; background: var(--vp-c-brand-1); display: inline-block; }
.visual-title { font-size: 13px; font-weight: 650; color: var(--vp-c-text-1); }
.visual-pill { margin-left: auto; font-size: 11px; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; color: var(--vp-c-text-3); border: 1px solid var(--vp-c-divider); padding: 2px 8px; border-radius: 999px; }
.visual-metric { display: flex; flex-direction: column; gap: 2px; margin-bottom: 16px; }
.visual-metric-label { font-size: 11px; letter-spacing: 0.08em; text-transform: uppercase; font-weight: 600; color: var(--vp-c-text-3); }
.visual-metric-value { font-size: 28px; font-weight: 700; letter-spacing: -0.02em; color: var(--vp-c-text-1); line-height: 1; }
.visual-metric-sub { font-size: 12.5px; color: #15803D; font-weight: 600; }
.dark .visual-metric-sub { color: #6FCA8A; }
.visual-bars { display: flex; align-items: end; gap: 6px; height: 48px; margin-bottom: 14px; }
.visual-bars .bar { flex: 1; display: flex; align-items: end; background: transparent; }
.visual-bars .bar i { display: block; width: 100%; height: 100%; border-radius: 4px 4px 0 0; opacity: 0.9; }
.visual-foot { display: flex; gap: 8px; flex-wrap: wrap; }
.visual-chip { font-size: 11px; font-weight: 600; padding: 4px 8px; border-radius: 999px; border: 1px solid var(--vp-c-divider); color: var(--vp-c-text-2); background: var(--vp-c-bg-soft); }
.visual-chip.accent { background: var(--vp-c-brand-soft); border-color: transparent; color: var(--vp-c-brand-1); }
.visual-copilot { margin-top: 14px; display: flex; gap: 10px; align-items: flex-start; padding: 10px 12px; border-radius: 10px; background: var(--vp-c-bg-soft); border: 1px solid var(--vp-c-divider); font-size: 13px; line-height: 1.5; color: var(--vp-c-text-2); }
.dark .visual-copilot { background: #15151B; }
.vc-icon { color: var(--vp-c-brand-1); font-size: 14px; flex: none; margin-top: 1px; }
.small-head { font-size: 12px; font-weight: 650; letter-spacing: 0.02em; color: var(--vp-c-text-1); margin-bottom: 12px; }
.small-row { display: flex; justify-content: space-between; font-size: 12.5px; color: var(--vp-c-text-2); margin-bottom: 6px; }
.small-row em { font-style: normal; font-weight: 650; color: var(--vp-c-text-1); margin-right: 6px; font-variant-numeric: tabular-nums; }
.small-bar { height: 4px; border-radius: 999px; background: var(--vp-c-divider); overflow: hidden; margin-bottom: 10px; }
.small-bar span { display: block; height: 100%; border-radius: 999px; background: var(--vp-c-brand-1); }
.small-bar.warn span { background: #FBBF24; }
.small-bar.ok span { background: #15803D; }
.dark .small-bar.ok span { background: #6FCA8A; }

/* Category color vars for bars */
:root { --c-housing: #A78BFA; --c-groceries: #34D399; --c-dining: #FB923C; --c-transport: #60A5FA; --c-subs: #F472B6; --c-health: #2DD4BF; }
</style>

