<template>
  <div class="feature-card">
    <div class="feature-icon" aria-hidden="true">
      <slot name="icon">
        <Icon v-if="icon" :name="icon" />
        <span v-else aria-hidden="true">·</span>
      </slot>
    </div>
    <h3 class="feature-title">{{ title }}</h3>
    <p class="feature-desc">{{ description }}</p>
    <a v-if="link" :href="href" class="feature-link">{{ linkText || "Learn more →" }}</a>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { withBase } from "vitepress";
import Icon from "./Icon.vue";
const props = defineProps<{
  title: string;
  description: string;
  icon?: string;
  link?: string;
  linkText?: string;
}>();
const href = computed(() => {
  if (!props.link) return undefined;
  if (props.link.startsWith("http") || props.link.startsWith("#")) return props.link;
  return withBase(props.link);
});
</script>

<style scoped>
.feature-card {
  border: 1px solid var(--vp-c-divider);
  border-radius: 14px;
  background: var(--vp-c-bg-elv);
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  transition: border-color 140ms, transform 140ms;
}
.dark .feature-card { background: #101015; border-color: rgba(255,255,255,0.06); }
.feature-card:hover { border-color: var(--vp-c-brand-soft); transform: translateY(-1px); }
.feature-icon {
  width: 36px; height: 36px;
  border-radius: 10px;
  display: flex; align-items: center; justify-content: center;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  color: var(--vp-c-text-2);
  font-size: 16px;
  transition: background 140ms, border-color 140ms, color 140ms;
}
.dark .feature-icon { background: #15151B; }
.feature-card:hover .feature-icon { color: var(--vp-c-brand-1); background: var(--vp-c-brand-soft); border-color: transparent; }
.feature-title {
  margin: 0;
  font-size: 15px;
  font-weight: 650;
  letter-spacing: -0.01em;
  color: var(--vp-c-text-1);
}
.feature-desc {
  margin: 0;
  font-size: 13.5px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}
.feature-link {
  margin-top: 4px;
  font-size: 13px;
  font-weight: 600;
  color: var(--vp-c-brand-1);
  text-decoration: none;
}
.feature-link:hover { text-decoration: underline; text-underline-offset: 3px; }
</style>

