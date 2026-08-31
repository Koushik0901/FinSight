<template>
  <ol class="step-list">
    <li v-for="(step, i) in steps" :key="i" class="step-item">
      <span class="step-num" aria-hidden="true">{{ i + 1 }}</span>
      <div class="step-body">
        <div class="step-title">{{ step.title }}</div>
        <div v-if="step.desc" class="step-desc">{{ step.desc }}</div>
      </div>
    </li>
  </ol>
</template>

<script setup lang="ts">
defineProps<{ steps: { title: string; desc?: string }[] }>();
</script>

<style scoped>
.step-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 12px; counter-reset: steps; }
.step-item { display: flex; gap: 14px; align-items: flex-start; border: 1px solid var(--vp-c-divider); border-radius: 12px; padding: 14px 16px; background: var(--vp-c-bg-elv); }
.dark .step-item { background: #101015; border-color: rgba(255,255,255,0.06); }
.step-num { flex: none; width: 28px; height: 28px; border-radius: 999px; display: inline-flex; align-items: center; justify-content: center; font-size: 12px; font-weight: 700; background: var(--vp-c-brand-soft); color: var(--vp-c-brand-1); border: 1px solid transparent; }
.dark .step-num { background: rgba(201,249,80,0.14); color: #C9F950; }
.step-title { font-size: 14px; font-weight: 620; color: var(--vp-c-text-1); line-height: 1.4; }
.step-desc { margin-top: 4px; font-size: 13px; line-height: 1.6; color: var(--vp-c-text-2); }
</style>

