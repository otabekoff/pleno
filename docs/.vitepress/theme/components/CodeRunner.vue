<script setup lang="ts">
// An editor with a Run button and an output panel. Pleno runs in the browser
// (WebAssembly in a Web Worker); nothing is sent to a server.
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import type { EditorView } from '@codemirror/view'
import { runPleno, type RunResult } from '../runner/run'

const props = withDefaults(
  defineProps<{
    code: string
    /** Editor height, as CSS. */
    height?: string
    /** `stack`: output under the editor. `split`: side by side on wide screens. */
    layout?: 'stack' | 'split'
    /** Title in the editor's header bar. */
    filename?: string
    /** Run once when the component appears. */
    autoRun?: boolean
  }>(),
  { height: '300px', layout: 'stack', filename: 'main.pln', autoRun: false },
)

const emit = defineEmits<{ (e: 'change', code: string): void }>()

const host = ref<HTMLElement>()
const view = shallowRef<EditorView>()
const running = ref(false)
const result = ref<RunResult | null>(null)

function currentCode(): string {
  return view.value?.state.doc.toString() ?? props.code
}

async function run() {
  if (running.value) return
  running.value = true
  try {
    result.value = await runPleno(currentCode())
  } finally {
    running.value = false
  }
}

/** Replaces the editor's text (the tutorial's "Show me" uses this). */
function setCode(code: string) {
  const v = view.value
  if (!v) return
  v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: code } })
  result.value = null
}

defineExpose({ run, setCode, getCode: currentCode })

onMounted(async () => {
  const [{ EditorState }, viewModule, commands, language, lang] = await Promise.all([
    import('@codemirror/state'),
    import('@codemirror/view'),
    import('@codemirror/commands'),
    import('@codemirror/language'),
    import('../editor/pleno-language'),
  ])
  const { EditorView: View, keymap, lineNumbers, drawSelection, highlightActiveLine, highlightActiveLineGutter } = viewModule

  view.value = new View({
    parent: host.value!,
    state: EditorState.create({
      doc: props.code,
      extensions: [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightActiveLine(),
        drawSelection(),
        commands.history(),
        language.indentOnInput(),
        language.bracketMatching(),
        lang.plenoLanguage,
        lang.plenoHighlighting,
        EditorState.tabSize.of(4),
        keymap.of([
          { key: 'Mod-Enter', run: () => (run(), true) },
          commands.indentWithTab,
          ...commands.defaultKeymap,
          ...commands.historyKeymap,
        ]),
        View.updateListener.of((update) => {
          if (update.docChanged) emit('change', update.state.doc.toString())
        }),
      ],
    }),
  })
  if (props.autoRun) run()
})

watch(
  () => props.code,
  (code) => {
    if (code !== currentCode()) setCode(code)
  },
)

onBeforeUnmount(() => view.value?.destroy())
</script>

<template>
  <div class="code-runner" :class="layout">
    <div class="panel editor-panel">
      <div class="bar">
        <span class="dots" aria-hidden="true"><i /><i /><i /></span>
        <span class="filename">{{ filename }}</span>
        <slot name="actions" />
        <button class="run" :disabled="running" title="Run (Ctrl+Enter)" @click="run">
          <span class="play" aria-hidden="true">▶</span>
          {{ running ? 'Running…' : 'Run' }}
        </button>
      </div>
      <div ref="host" class="editor" :style="{ height }">
        <!-- Shown until the editor loads, and to search engines. -->
        <pre v-if="!view" class="fallback">{{ code }}</pre>
      </div>
    </div>
    <div class="panel output-panel" :class="{ failed: result && !result.ok }">
      <div class="bar">
        <span class="filename">Output</span>
        <span v-if="result" class="status">
          {{ result.ok ? 'Done' : 'Error' }} · {{ result.elapsed }} ms
        </span>
      </div>
      <div class="output" :style="layout === 'split' ? { height } : undefined" aria-live="polite">
        <p v-if="!result" class="hint">
          {{ running ? 'Starting Pleno…' : 'Press Run or Ctrl+Enter. The code runs in your browser.' }}
        </p>
        <template v-else>
          <pre v-if="result.output" class="stdout">{{ result.output }}</pre>
          <pre v-if="result.error" class="stderr">{{ result.error }}</pre>
          <p v-if="!result.output && !result.error" class="hint">The program printed nothing.</p>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.code-runner {
  display: grid;
  gap: 12px;
  text-align: left;
}

@media (min-width: 960px) {
  .code-runner.split {
    grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
  }
}

.panel {
  min-width: 0;
  border: 1px solid var(--pl-panel-border);
  border-radius: var(--pl-radius);
  background: var(--pl-panel);
  backdrop-filter: blur(10px);
  overflow: hidden;
  box-shadow: 0 10px 40px -20px rgba(0, 0, 0, 0.45);
}

.bar {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 40px;
  padding: 0 8px 0 14px;
  border-bottom: 1px solid var(--vp-c-divider);
  font-size: 12px;
  color: var(--vp-c-text-2);
}

.dots {
  display: inline-flex;
  gap: 6px;
}

.dots i {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--vp-c-default-3);
}

.dots i:nth-child(1) { background: #ef4857; }
.dots i:nth-child(2) { background: #d211ec; }
.dots i:nth-child(3) { background: #7f52ff; }

.filename {
  font-family: var(--vp-font-family-mono);
  margin-right: auto;
}

.status {
  font-family: var(--vp-font-family-mono);
  color: var(--vp-c-text-3);
}

.run {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 14px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 600;
  color: #fff;
  background: var(--pl-gradient);
  transition: filter 0.2s, opacity 0.2s;
}

.run:hover {
  filter: brightness(1.1);
}

.run:disabled {
  opacity: 0.6;
  cursor: progress;
}

.play {
  font-size: 10px;
}

.editor {
  overflow: auto;
  font-size: 13.5px;
}

.editor :deep(.cm-editor) {
  height: 100%;
  background: transparent;
  color: var(--vp-c-text-1);
}

.editor :deep(.cm-editor.cm-focused) {
  outline: none;
}

.editor :deep(.cm-scroller) {
  font-family: var(--vp-font-family-mono);
  line-height: 1.65;
}

.editor :deep(.cm-gutters) {
  background: transparent;
  border: none;
  color: var(--vp-c-text-3);
}

.editor :deep(.cm-activeLine),
.editor :deep(.cm-activeLineGutter) {
  background: color-mix(in srgb, var(--vp-c-brand-1) 7%, transparent);
}

.editor :deep(.cm-cursor) {
  border-left-color: var(--vp-c-brand-1);
}

.editor :deep(.cm-selectionBackground),
.editor :deep(.cm-focused .cm-selectionBackground) {
  background: rgba(127, 82, 255, 0.28) !important;
}

.editor :deep(.cm-matchingBracket) {
  background: rgba(127, 82, 255, 0.22);
  outline: none;
}

.fallback {
  margin: 0;
  padding: 4px 16px;
  font-family: var(--vp-font-family-mono);
  font-size: 13.5px;
  line-height: 1.65;
  white-space: pre;
  color: var(--vp-c-text-2);
}

.output {
  min-height: 72px;
  max-height: 360px;
  overflow: auto;
  padding: 12px 16px;
}

.output pre {
  margin: 0;
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.stderr {
  margin-top: 8px !important;
  color: var(--vp-c-danger-1);
}

.failed {
  border-color: color-mix(in srgb, var(--vp-c-danger-1) 35%, transparent);
}

.hint {
  margin: 0;
  font-size: 13px;
  color: var(--vp-c-text-3);
}
</style>
