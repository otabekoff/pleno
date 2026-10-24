<script setup lang="ts">
// The playground: pick an example or open a shared link (#code=...), edit,
// run, and share. Everything runs in the browser.
import { onBeforeUnmount, onMounted, ref } from 'vue'
import CodeRunner from './CodeRunner.vue'
import PlIcon from './PlIcon.vue'
import PlSelect from './PlSelect.vue'
import { decodeCode, encodeCode } from '../runner/run'
import { examples } from '../playground-examples'

const options = examples.map((e) => ({ value: e.name, label: e.name }))
const code = ref(examples[0].code)
const selected = ref<string>(examples[0].name)
const shared = ref(false)
let latest = code.value
let timer: ReturnType<typeof setTimeout> | undefined

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
    clearTimeout(timer)
    timer = setTimeout(() => (shared.value = false), 1800)
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
      <div class="picker">
        <span class="picker-label" aria-hidden="true">Example</span>
        <PlSelect
          :model-value="selected"
          :options="options"
          label="Example"
          placeholder="Shared code"
          @update:model-value="choose"
        />
      </div>
    </header>
    <CodeRunner
      :code="code"
      layout="split"
      height="min(62vh, 640px)"
      filename="playground.pln"
      @change="latest = $event"
    >
      <template #actions>
        <button type="button" class="pl-btn pl-btn-ghost pl-btn-sm" @click="share">
          <PlIcon :name="shared ? 'check' : 'link'" :size="14" />
          {{ shared ? 'Link copied' : 'Share' }}
        </button>
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
  max-width: 1440px;
  margin: 0 auto;
  padding: 32px 24px 48px;
}

@media (min-width: 768px) {
  .playground {
    padding: 40px 48px 56px;
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
  font-size: 28px;
  font-weight: 700;
}

.head p {
  margin: 6px 0 0;
  font-size: 15px;
  color: var(--vp-c-text-2);
}

.picker {
  display: flex;
  align-items: center;
  gap: 10px;
}

.picker :deep(.pl-select) {
  width: 220px;
}

.picker-label {
  font-size: 13px;
  color: var(--vp-c-text-2);
}

.note {
  margin-top: 16px;
  font-size: 13px;
  color: var(--vp-c-text-3);
}
</style>
