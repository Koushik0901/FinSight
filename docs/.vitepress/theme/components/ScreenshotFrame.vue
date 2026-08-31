<template>
  <figure class="shot">
    <div class="shot-frame">
      <div class="shot-bar" aria-hidden="true">
        <span class="traffic" />
        <span class="traffic" />
        <span class="traffic" />
        <span class="shot-bar-title">{{ title || "FinSight" }}</span>
      </div>
      <img :src="src" :alt="alt" loading="lazy" decoding="async" class="shot-img" />
    </div>
    <figcaption v-if="caption" class="shot-caption">{{ caption }}</figcaption>
  </figure>
</template>

<script setup lang="ts">
defineProps<{ src: string; alt: string; caption?: string; title?: string }>();
</script>

<style scoped>
.shot { margin: 0; }
.shot-frame { border:1px solid var(--vp-c-divider); border-radius:14px; overflow:hidden; background: var(--vp-c-bg-elv); box-shadow: 0 1px 2px rgba(0,0,0,0.06), 0 16px 40px -16px rgba(0,0,0,0.12); }
.dark .shot-frame { background:#101015; border-color: rgba(255,255,255,0.06); box-shadow: 0 1px 0 rgba(255,255,255,0.04) inset, 0 20px 48px rgba(0,0,0,0.32); }
.shot-bar { display:flex; align-items:center; gap:6px; padding:10px 12px; border-bottom:1px solid var(--vp-c-divider); background: var(--vp-c-bg-soft); }
.dark .shot-bar { background:#0B0B0F; }
.traffic { width:10px; height:10px; border-radius:999px; background: var(--vp-c-divider); display:inline-block; }
.traffic:nth-child(1){ background:#FF5F57; opacity:0.9; }
.traffic:nth-child(2){ background:#FFBD2E; opacity:0.9; }
.traffic:nth-child(3){ background:#28CA41; opacity:0.9; }
.shot-bar-title { margin-left:auto; font-size:11px; font-weight:600; letter-spacing:0.04em; color:var(--vp-c-text-3); font-family: var(--fs-mono); }
.shot-img { display:block; width:100%; height:auto; }
.shot-caption { margin-top:8px; font-size:12.5px; color:var(--vp-c-text-3); line-height:1.5; text-align:center; }
</style>

