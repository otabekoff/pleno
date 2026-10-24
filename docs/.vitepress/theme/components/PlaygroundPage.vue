<script setup lang="ts">
// The playground: pick an example or open a shared link (#code=...), edit,
// run, and share. Everything runs in the browser.
import { onBeforeUnmount, onMounted, ref } from 'vue'
import CodeRunner from './CodeRunner.vue'
import { decodeCode, encodeCode } from '../runner/run'
import { examples } from '../playground-examples'

const code = ref(examples[0].code)
const selected = ref(examples[0].name)
const shared = ref(false)
let latest = code.value

function openLink() {
  const match = location.hash.match(/^#code=(.+)$/)
  const fromLink = match && decodeCode(match[1])
  if (fromLink) {
    code.value = fromLink
    latest = fromLink
    selected.value = ''
  }
}

onMounted(() => {
  openLink()
  window.addEventListener('hashchange', openLink)
})
onBeforeUnmount(() => window.removeEventListener('hashchange', openLink))

function choose(name: string) {
  const example = examples.find((e) => e.name === name)
  if (!example) return
  selected.value = name
  code.value = example.code
  latest = example.code
  history.replaceState(null, '', location.pathname)
}

async function share() {
  const url = `${location.origin}${location.pathname}#code=${encodeCode(latest)}`
  history.replaceState(null, '', url)
  try {
    await navigator.clipboard.writeText(url)
    shared.value = true
    setTimeout(() => (shared.value = false), 1800)
  } catch {
    // The link is in the address bar either way.
  }
}
</script>

<template>
  <div class="playground">
    <header class="head">
      <div>
        <h1 class="pl-display">Playground</h1>
        <p>Pleno compiled to WebAssembly, running in your browser. Nothing is sent to a server.</p>
      </div>
      <label class="picker">
        <span>Example</span>
        <select :value="selected" @change="choose(($event.target as HTMLSelectElement).value)">
          <option v-if="!selected" value="" disabled>Shared code</option>
          <option v-for="example in examples" :key="example.name" :value="example.name">
            {{ example.name }}
          </option>
        </select>
      </label>
    </header>
    <CodeRunner
      :code="code"
      layout="split"
      height="min(62vh, 640px)"
      filename="playground.pln"
      @change="latest = $event"
    >
      <template #actions>
        <button class="share" @click="share">{{ shared ? 'Link copied' : 'Share' }}</button>
      </template>
    </CodeRunner>
    <p class="note">
      File, network and process functions are not available in the browser. Programs stop after
      50 million instructions or 10 seconds.
    </p>
  </div>
</template>

<style scoped>
.playground {
  max-width: 1400px;
  margin: 0 auto;
  padding: 32px 24px 48px;
}

@media (min-width: 768px) {
  .playground {
    padding: 40px 48px 64px;
  }
}

.head {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;
}

.head h1 {
  margin: 0;
  font-size: 32px;
  font-weight: 700;
}

.head p {
  margin: 6px 0 0;
  color: var(--vp-c-text-2);
}

.picker {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
  color: var(--vp-c-text-2);
}

.picker select {
  padding: 6px 12px;
  border: 1px solid var(--vp-c-border);
  border-radius: 999px;
  background: var(--vp-c-bg-soft);
  color: var(--vp-c-text-1);
  font-size: 14px;
}

.share {
  padding: 4px 12px;
  border: 1px solid var(--vp-c-border);
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
  color: var(--vp-c-text-1);
  transition: border-color 0.2s;
}

.share:hover {
  border-color: var(--vp-c-brand-2);
}

.note {
  margin-top: 16px;
  font-size: 13px;
  color: var(--vp-c-text-3);
}
</style>
