<script setup lang="ts">
// A one-line banner above the navbar. Dismissing it is remembered in this
// browser; the layout's top offset follows its height.
import { onMounted, ref, watchEffect } from 'vue'
import { withBase } from 'vitepress'

const KEY = 'pleno-announcement-1.0-road'
const visible = ref(false)

onMounted(() => {
  try {
    visible.value = localStorage.getItem(KEY) !== 'dismissed'
  } catch {
    visible.value = true
  }
})

watchEffect(() => {
  if (typeof document === 'undefined') return
  document.documentElement.style.setProperty('--vp-layout-top-height', visible.value ? '36px' : '0px')
})

function dismiss() {
  visible.value = false
  try {
    localStorage.setItem(KEY, 'dismissed')
  } catch {
    // Storage can be unavailable; the banner just comes back next time.
  }
}
</script>

<template>
  <div v-if="visible" class="announcement">
    <a :href="withBase('/blog/toward-1-0')">
      <span class="tag">New</span>
      Every example in these docs now runs. Read about the road to Pleno 1.0
      <span aria-hidden="true">→</span>
    </a>
    <button class="close" aria-label="Dismiss" @click="dismiss">×</button>
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
  height: 36px;
  padding: 0 44px;
  background: var(--pl-gradient);
  font-size: 13px;
  font-weight: 500;
  color: #fff;
}

.announcement a {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: #fff;
}

.announcement a:hover {
  text-decoration: underline;
  text-underline-offset: 3px;
}

.tag {
  padding: 1px 8px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.22);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.close {
  position: absolute;
  right: 12px;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  font-size: 18px;
  line-height: 1;
  color: #fff;
  opacity: 0.85;
}

.close:hover {
  opacity: 1;
  background: rgba(0, 0, 0, 0.18);
}
</style>
