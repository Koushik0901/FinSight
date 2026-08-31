<template>
  <div class="provider-card">
    <div class="provider-head">
      <span class="provider-icon" aria-hidden="true">
        <Icon v-if="isIconName" :name="icon!" />
        <span v-else>{{ icon }}</span>
      </span>
      <div>
        <div class="provider-title">{{ title }}</div>
        <div class="provider-subtitle">{{ subtitle }}</div>
      </div>
      <span v-if="badge" class="provider-badge">{{ badge }}</span>
    </div>
    <p class="provider-desc">{{ description }}</p>
    <ul v-if="bullets && bullets.length" class="provider-bullets">
      <li v-for="(b, i) in bullets" :key="i">{{ b }}</li>
    </ul>
    <a v-if="link" :href="link" class="provider-link">{{ linkText || "Learn more →" }}</a>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import Icon from "./Icon.vue";
const props = defineProps<{
  icon?: string;
  title: string;
  subtitle?: string;
  description: string;
  bullets?: string[];
  badge?: string;
  link?: string;
  linkText?: string;
}>();
const known = new Set(["today","wallet","grid","repeat","recipe","goal","journey","lego","gear","search","horizon","info","lock","eye","bolt","spark","brain","cpu","house","sparkle","bulb","box","flow","bell","cart","tag","pencil","check","plus","x"]);
const isIconName = computed(() => !!props.icon && known.has(props.icon));
</script>

<style scoped>
.provider-card { border: 1px solid var(--vp-c-divider); border-radius: 14px; background: var(--vp-c-bg-elv); padding: 18px; display:flex; flex-direction: column; gap: 10px; }
.dark .provider-card { background:#101015; border-color: rgba(255,255,255,0.06); }
.provider-head { display:flex; gap:12px; align-items:center; }
.provider-icon { width:36px; height:36px; border-radius:10px; display:flex; align-items:center; justify-content:center; background: var(--vp-c-bg-soft); border:1px solid var(--vp-c-divider); font-size:16px; flex:none; }
.dark .provider-icon { background:#15151B; }
.provider-title { font-size:14px; font-weight:650; color:var(--vp-c-text-1); line-height:1.2; }
.provider-subtitle { font-size:12px; color:var(--vp-c-text-3); margin-top:2px; }
.provider-badge { margin-left:auto; font-size:11px; font-weight:700; letter-spacing:0.06em; text-transform:uppercase; padding:4px 8px; border-radius:999px; background: var(--vp-c-brand-soft); color: var(--vp-c-brand-1); border:1px solid transparent; flex:none; }
.provider-desc { margin:0; font-size:13.5px; line-height:1.6; color:var(--vp-c-text-2); }
.provider-bullets { margin:0; padding-left:18px; font-size:13px; line-height:1.6; color:var(--vp-c-text-2); }
.provider-link { font-size:13px; font-weight:600; color:var(--vp-c-brand-1); text-decoration:none; margin-top:2px; }
.provider-link:hover { text-decoration: underline; text-underline-offset:3px; }
</style>

