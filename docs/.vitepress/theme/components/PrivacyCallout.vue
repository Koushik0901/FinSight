<template>
  <div class="privacy-callout">
    <div class="privacy-head">
      <span class="privacy-icon" aria-hidden="true"><Icon name="lock" /></span>
      <span class="privacy-title">Privacy note</span>
    </div>
    <p class="privacy-text"><slot /></p>
  </div>
</template>

<script setup lang="ts">
import Icon from "./Icon.vue";
</script>

<style scoped>
.privacy-callout {
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  padding: 16px 18px;
  background: var(--vp-c-bg-soft);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.dark .privacy-callout { background: #101015; border-color: rgba(255,255,255,0.06); }
.privacy-head { display: flex; align-items: center; gap: 8px; }
.privacy-icon { color: var(--vp-c-brand-1); display: inline-flex; }
.privacy-icon :deep(svg) { width: 14px; height: 14px; }
.privacy-title { font-size: 12px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; color: var(--vp-c-text-1); }
.privacy-text { margin: 0; font-size: 13.5px; line-height: 1.6; color: var(--vp-c-text-2); }
</style>
