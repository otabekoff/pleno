<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import PlIcon from './PlIcon.vue'

const platforms = {
  unix: {
    label: 'macOS / Linux',
    prompt: '$',
    command: 'curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh',
  },
  windows: {
    label: 'Windows',
    prompt: '>',
    command: 'irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.ps1 | iex',
  },
} as const

type Platform = keyof typeof platforms

const platform = ref<Platform>('unix')
const copied = ref(false)
const current = computed(() => platforms[platform.value])
let timer: ReturnType<typeof setTimeout> | undefined

onMounted(() => {
  if (navigator.userAgent.includes('Windows')) platform.value = 'windows'
})

async function copy() {
  try {
    await navigator.clipboard.writeText(current.value.command)
    copied.value = true
    clearTimeout(timer)
    timer = setTimeout(() => (copied.value = false), 1600)
  } catch {
    // Clipboard access can be denied; the command can still be selected.
  }
}
</script>

<template>
  <div class="install">
    <div class="head">
      <div class="segmented" role="tablist" aria-label="Operating system">
        <button
          v-for="(info, key) in platforms"
          :key="key"
          type="button"
          role="tab"
          :aria-selected="platform === key"
          :class="{ active: platform === key }"
          @click="platform = key"
        >
          {{ info.label }}
        </button>
      </div>
      <button
        type="button"
        class="pl-btn pl-btn-ghost pl-btn-sm copy"
        :aria-label="copied ? 'Copied' : 'Copy the install command'"
        @click="copy"
      >
        <PlIcon :name="copied ? 'check' : 'copy'" :size="14" />
        <span>{{ copied ? 'Copied' : 'Copy' }}</span>
      </button>
    </div>
    <div class="command" role="tabpanel">
      <span class="prompt" aria-hidden="true">{{ current.prompt }}</span>
      <code>{{ current.command }}</code>
    </div>
  </div>
</template>

<style scoped>
.install {
  width: 100%;
  max-width: 560px;
  overflow: hidden;
  border: 1px solid var(--vp-c-border);
  border-radius: var(--pl-radius-lg);
  background-color: var(--vp-code-block-bg);
  text-align: left;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 6px 6px 8px;
  border-bottom: 1px solid var(--vp-c-divider);
  background-color: var(--vp-c-bg);
}

.segmented {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border-radius: var(--pl-radius);
  background-color: var(--vp-c-bg-soft);
}

.segmented button {
  height: 26px;
  padding: 0 10px;
  border-radius: var(--pl-radius-sm);
  font-size: 13px;
  font-weight: 500;
  color: var(--vp-c-text-2);
  transition: color 0.15s, background-color 0.15s, box-shadow 0.15s;
}

.segmented button:hover {
  color: var(--vp-c-text-1);
}

.segmented button.active {
  color: var(--vp-c-text-1);
  background-color: var(--vp-c-bg);
  box-shadow: 0 0 0 1px var(--vp-c-border);
}

.dark .segmented button.active {
  background-color: var(--vp-c-bg-elv);
}

.copy {
  color: var(--vp-c-text-2);
}

.command {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 46px;
  padding: 0 16px;
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
}

.prompt {
  flex-shrink: 0;
  color: var(--vp-c-text-3);
  user-select: none;
}

/* The long command scrolls sideways without showing a scrollbar; the fade
   at the right edge says there is more. */
.command code {
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  white-space: nowrap;
  color: var(--vp-c-text-1);
  scrollbar-width: none;
  -webkit-mask-image: linear-gradient(to right, #000 calc(100% - 40px), transparent);
  mask-image: linear-gradient(to right, #000 calc(100% - 40px), transparent);
}

.command code::-webkit-scrollbar {
  display: none;
}
</style>
