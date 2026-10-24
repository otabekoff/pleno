<script setup lang="ts">
// Stroke icons drawn on a 24px grid (shapes from Lucide, ISC license).
const paths = {
  play: '<path d="M7 4.5v15l12-7.5z"/>',
  copy: '<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M4 16a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
  'chevron-down': '<path d="m6 9 6 6 6-6"/>',
  'chevron-left': '<path d="m15 18-6-6 6-6"/>',
  'chevron-right': '<path d="m9 18 6-6-6-6"/>',
  'arrow-right': '<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>',
  x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
  reset: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
  eye: '<path d="M2.06 12.35a1 1 0 0 1 0-.7 10.75 10.75 0 0 1 19.88 0 1 1 0 0 1 0 .7 10.75 10.75 0 0 1-19.88 0"/><circle cx="12" cy="12" r="3"/>',
  spinner: '<path d="M21 12a9 9 0 1 1-6.22-8.56"/>',
  terminal: '<path d="m4 17 6-6-6-6"/><path d="M12 19h8"/>',
} as const

export type IconName = keyof typeof paths

withDefaults(defineProps<{ name: IconName; size?: number }>(), { size: 16 })
</script>

<template>
  <svg
    class="pl-icon"
    :class="`pl-icon-${name}`"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
    v-html="paths[name]"
  />
</template>

<style scoped>
.pl-icon {
  flex-shrink: 0;
}

.pl-icon-play {
  fill: currentColor;
  stroke-width: 1.5;
}

.pl-icon-spinner {
  animation: pl-spin 0.8s linear infinite;
}

@keyframes pl-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
