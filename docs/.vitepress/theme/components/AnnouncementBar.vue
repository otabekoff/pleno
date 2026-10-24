<script setup lang="ts">
// A one-line banner above the navbar. It is part of the server-rendered page;
// when it was dismissed, a script in <head> (config.ts) adds
// `pl-banner-dismissed` to <html> before the first paint, which hides it and
// sets the layout's top offset to 0, so nothing moves while the page loads.
import { withBase } from 'vitepress'
import PlIcon from './PlIcon.vue'

// Also read by the script in <head> (config.ts).
const BANNER_KEY = 'pleno-announcement-1.0-road'

function dismiss() {
  document.documentElement.classList.add('pl-banner-dismissed')
  try {
    localStorage.setItem(BANNER_KEY, 'dismissed')
  } catch {
    // Storage can be unavailable; the banner comes back on the next visit.
  }
}
</script>

<template>
  <div class="announcement">
    <a :href="withBase('/blog/toward-1-0')">
      <span class="tag">New</span>
      <span class="text">Every example in these docs now runs. Read about the road to Pleno 1.0</span>
      <PlIcon name="arrow-right" :size="14" />
    </a>
    <button class="close" type="button" aria-label="Dismiss announcement" @click="dismiss">
      <PlIcon name="x" :size="16" />
    </button>
  </div>
</template>

<style scoped>
.announcement {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: var(--vp-z-index-layout-top, 40);
  display: flex;
  align-items: center;
  justify-content: center;
  height: var(--pl-banner-height);
  padding: 0 48px;
  overflow: hidden;
  border-bottom: 1px solid var(--vp-c-divider);
  background-color: var(--vp-c-bg-alt);
  font-size: 13px;
}

:global(html.pl-banner-dismissed) .announcement {
  display: none;
}

.announcement a {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  color: var(--vp-c-text-1);
  font-weight: 500;
  text-decoration: none;
}

.announcement a:hover .text {
  text-decoration: underline;
  text-underline-offset: 3px;
}

.announcement a .pl-icon {
  color: var(--vp-c-text-3);
  transition: transform 0.15s;
}

.announcement a:hover .pl-icon {
  transform: translateX(2px);
}

.text {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.tag {
  flex-shrink: 0;
  padding: 2px 7px;
  border-radius: 4px;
  background-color: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
  font-size: 11px;
  font-weight: 600;
}

.close {
  position: absolute;
  right: 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: var(--pl-radius-sm);
  color: var(--vp-c-text-3);
  transition: background-color 0.15s, color 0.15s;
}

.close:hover {
  color: var(--vp-c-text-1);
  background-color: var(--vp-c-default-soft);
}
</style>
