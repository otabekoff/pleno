<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

const commands = {
  unix: 'curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh',
  windows: 'irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.ps1 | iex',
} as const

type Platform = keyof typeof commands

const platform = ref<Platform>('unix')
const copied = ref(false)
const command = computed(() => commands[platform.value])

onMounted(() => {
  if (navigator.userAgent.includes('Windows')) platform.value = 'windows'
})

async function copy() {
  try {
    await navigator.clipboard.writeText(command.value)
    copied.value = true
    setTimeout(() => (copied.value = false), 1500)
  } catch {
    // Clipboard access can be denied; the command is still selectable.
  }
}
</script>

<template>
  <div class="home-install">
    <div class="tabs" role="tablist" aria-label="Operating system">
      <button
        role="tab"
        :aria-selected="platform === 'unix'"
        :class="{ active: platform === 'unix' }"
        @click="platform = 'unix'"
      >
        macOS / Linux
      </button>
      <button
        role="tab"
        :aria-selected="platform === 'windows'"
        :class="{ active: platform === 'windows' }"
        @click="platform = 'windows'"
      >
        Windows
      </button>
    </div>
    <div class="command">
      <span class="prompt" aria-hidden="true">{{ platform === 'windows' ? '>' : '$' }}</span>
      <code>{{ command }}</code>
      <button class="copy" :aria-label="copied ? 'Copied' : 'Copy command'" @click="copy">
        {{ copied ? 'Copied' : 'Copy' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
/* Centered under the hero on narrow screens, left-aligned beside the logo
   on wide ones, and never wider than the screen. */
.home-install {
  /* A viewport-based limit: the hero grows to fit its content, so a
     percentage would let the long command widen the page. */
  width: min(640px, calc(100vw - 48px));
  margin: 28px auto 0;
}

.tabs {
  display: flex;
  justify-content: center;
  gap: 4px;
  margin-bottom: 8px;
}

@media (min-width: 960px) {
  .home-install {
    margin: 28px 0 0;
  }

  .tabs {
    justify-content: flex-start;
  }
}

.tabs button {
  padding: 2px 10px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 500;
  color: var(--vp-c-text-2);
  transition: color 0.2s, background-color 0.2s;
}

.tabs button:hover {
  color: var(--vp-c-text-1);
}

.tabs button.active {
  color: var(--vp-c-brand-1);
  background-color: var(--vp-c-brand-soft);
}

.command {
  display: flex;
  max-width: 100%;
  text-align: left;
  align-items: center;
  gap: 10px;
  padding: 10px 10px 10px 16px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  background-color: var(--vp-c-bg-soft);
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
}

.prompt {
  color: var(--vp-c-brand-1);
  user-select: none;
}

.command code {
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  white-space: nowrap;
  color: var(--vp-c-text-1);
}

.copy {
  flex-shrink: 0;
  padding: 4px 12px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--vp-c-white);
  background: var(--pleno-gradient);
  transition: opacity 0.2s;
}

.copy:hover {
  opacity: 0.85;
}
</style>
