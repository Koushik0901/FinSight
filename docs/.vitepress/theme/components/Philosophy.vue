<template>
  <section class="philosophy">
    <div class="phil-inner">
      <div class="phil-copy">
        <p class="phil-kicker"><Icon name="journey" style="width:12px;height:12px;vertical-align:-1px" /> Financial framework</p>
        <h2 class="phil-title">Principles, not prescriptions.</h2>
        <p class="phil-text">FinSight is shaped by six ideas that have survived decades of advice — not as slogans, but as checks the Copilot actually applies to your numbers.</p>
        <ul class="phil-list">
          <li><Icon name="wallet" /><strong>Pay yourself first</strong> <span>— at least 10% before anything else.</span></li>
          <li><Icon name="search" /><strong>Conscious spending</strong> <span>— every category is a Need, Want, Saving, or Investment.</span></li>
          <li><Icon name="lock" /><strong>Emergency fund first</strong> <span>— 3–6 months before ambitious plans.</span></li>
          <li><Icon name="repeat" /><strong>Debt snowball</strong> <span>— smallest balance first, for momentum.</span></li>
          <li><Icon name="flow" /><strong>Compound growth</strong> <span>— 10/20/30-year projection from today’s rate.</span></li>
          <li><Icon name="spark" /><strong>Behaviour over math</strong> <span>— nudges about patterns, not just totals.</span></li>
        </ul>
        <a class="phil-link" :href="withBase('/framework/overview')">Explore the framework →</a>
      </div>
      <div class="phil-visual" aria-hidden="true">
        <div class="phil-card">
          <div class="phil-card-head">Conscious spending · March</div>
          <div class="donut-wrap">
            <div class="donut" />
            <div class="donut-legend">
              <span><i style="background:#60A5FA"/>Needs 52%</span>
              <span><i style="background:#FB923C"/>Wants 21%</span>
              <span><i style="background:#C9F950"/>Savings 27%</span>
            </div>
          </div>
          <div class="phil-note">The Copilot flags when wants quietly creep past needs.</div>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { withBase } from "vitepress";
</script>

<style scoped>
.philosophy { padding: 36px 0; border-top: 1px solid var(--vp-c-divider); border-bottom: 1px solid var(--vp-c-divider); background: var(--vp-c-bg-soft); }
.dark .philosophy { background: #0B0B0F; }
.phil-inner { max-width: 1152px; margin: 0 auto; padding: 0 24px; display: grid; grid-template-columns: 1.2fr 0.8fr; gap: 40px; align-items: center; }
@media (max-width: 860px) { .phil-inner { grid-template-columns: 1fr; } }
.phil-kicker { font-family: var(--fs-mono); font-size: 11px; letter-spacing: 0.09em; text-transform: uppercase; font-weight: 700; color: var(--vp-c-text-3); margin: 0 0 10px; display: inline-flex; align-items: center; gap: 6px; }
.phil-title { margin: 0; font-size: 28px; font-weight: 700; letter-spacing: -0.02em; line-height: 1.1; color: var(--vp-c-text-1); }
.phil-text { margin: 12px 0 0; font-size: 14.5px; line-height: 1.6; color: var(--vp-c-text-2); max-width: 52ch; }
.phil-list { list-style: none; padding: 0; margin: 20px 0 0; display: flex; flex-direction: column; gap: 8px; }
.phil-list li { font-size: 13.5px; line-height: 1.5; color: var(--vp-c-text-1); display: flex; gap: 8px; flex-wrap: wrap; align-items: flex-start; }
.phil-list li :deep(svg) { width: 14px; height: 14px; flex: none; color: var(--vp-c-text-3); margin-top: 2px; }
.phil-link { display: inline-block; margin-top: 18px; font-size: 14px; font-weight: 600; color: var(--vp-c-brand-1); text-decoration: none; }
.phil-link:hover { text-decoration: underline; text-underline-offset: 3px; }
.phil-card { border: 1px solid var(--vp-c-divider); border-radius: 16px; background: var(--vp-c-bg-elv); padding: 18px; }
.dark .phil-card { background: #101015; }
.phil-card-head { font-size: 11px; letter-spacing: 0.07em; text-transform: uppercase; font-weight: 700; color: var(--vp-c-text-3); margin-bottom: 16px; }
.donut-wrap { display: flex; align-items: center; gap: 20px; }
.donut { width: 96px; height: 96px; border-radius: 50%; background: conic-gradient(#60A5FA 0 52%, #FB923C 52% 73%, #C9F950 73% 100%); border: 8px solid var(--vp-c-bg-elv); flex: none; position: relative; }
.dark .donut { border-color: #101015; }
.donut::after { content:""; position:absolute; inset: 18px; border-radius:50%; background: var(--vp-c-bg-elv); }
.dark .donut::after { background: #101015; }
.donut-legend { display:flex; flex-direction: column; gap: 8px; font-size: 13px; color: var(--vp-c-text-2); }
.donut-legend span { display:flex; align-items:center; gap:8px; }
.donut-legend i { width:10px; height:10px; border-radius:999px; display:inline-block; }
.phil-note { margin-top: 14px; font-size: 12.5px; color: var(--vp-c-text-3); line-height: 1.5; border-top: 1px solid var(--vp-c-divider); padding-top: 12px; }
</style>
