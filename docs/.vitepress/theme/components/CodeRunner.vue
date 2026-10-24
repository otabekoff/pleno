<script setup lang="ts">
// An editor with a Run button and an output panel. Pleno runs in the browser
// (WebAssembly in a Web Worker); nothing is sent to a server. Until
// CodeMirror loads, the same code is shown highlighted with matching
// metrics, so the switch is not visible.
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import type { EditorView } from '@codemirror/view'
import { runPleno, type RunResult } from '../runner/run'
import PlCode from './PlCode.vue'
import PlIcon from './PlIcon.vue'

const props = withDefaults(
  defineProps<{
    code: string
    /** Editor height, as CSS. */
    height?: string
    /** `stack`: output under the editor. `split`: side by side on wide screens. */
    layout?: 'stack' | 'split'
    /** Title in the editor's header bar. */
    filename?: string
  }>(),
  { height: '300px', layout: 'stack', filename: 'main.pln' },
)

const emit = defineEmits<{ (e: 'change', code: string): void }>()

const host = ref<HTMLElement>()
const view = shallowRef<EditorView>()
const running = ref(false)
const result = ref<RunResult | null>(null)
const isMac = ref(false)

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
  isMac.value = /Mac|iPhone|iPad/.test(navigator.platform)
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
    <section class="pane editor-pane" aria-label="Code">
      <header class="bar">
        <span class="filename">{{ filename }}</span>
        <div class="actions">
          <slot name="actions" />
          <button
            class="pl-btn pl-btn-primary pl-btn-sm run"
            type="button"
            :disabled="running"
            :title="`Run (${isMac ? '⌘' : 'Ctrl'}+Enter)`"
            @click="run"
          >
            <PlIcon :name="running ? 'spinner' : 'play'" :size="12" />
            Run
          </button>
        </div>
      </header>
      <div ref="host" class="editor" :style="{ height }">
        <PlCode v-if="!view" class="fallback" :code="code" line-numbers />
      </div>
    </section>
    <section class="pane output-pane" :class="{ failed: result && !result.ok }" aria-label="Output">
      <header class="bar">
        <span class="label">Output</span>
        <span v-if="result" class="status" :class="result.ok ? 'ok' : 'error'">
          <PlIcon :name="result.ok ? 'check' : 'x'" :size="13" />
          {{ result.ok ? 'Finished' : 'Failed' }} in {{ result.elapsed }} ms
        </span>
      </header>
      <div class="output" :style="layout === 'split' ? { height } : undefined" aria-live="polite">
        <p v-if="!result" class="hint">
          <template v-if="running">Starting Pleno…</template>
          <template v-else>
            Press <kbd>Run</kbd> or <kbd>{{ isMac ? '⌘' : 'Ctrl' }}</kbd> <kbd>Enter</kbd>. The code runs in your browser.
          </template>
        </p>
        <template v-else>
          <pre v-if="result.output" class="stdout">{{ result.output }}</pre>
          <pre v-if="result.error" class="stderr">{{ result.error }}</pre>
          <p v-if="!result.output && !result.error" class="hint">The program printed nothing.</p>
        </template>
      </div>
    </section>
  </div>
</template>

<style scoped>
.code-runner {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--vp-c-border);
  border-radius: var(--pl-radius-lg);
  background-color: var(--vp-code-block-bg);
  box-shadow: var(--pl-shadow-panel);
  text-align: left;
}

@media (min-width: 960px) {
  .code-runner.split {
    grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
  }

  .split .output-pane {
    border-top: none;
    border-left: 1px solid var(--vp-c-divider);
  }
}

.pane {
  min-width: 0;
}

.output-pane {
  border-top: 1px solid var(--vp-c-divider);
}

.bar {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 44px;
  padding: 0 8px 0 16px;
  border-bottom: 1px solid var(--vp-c-divider);
  background-color: var(--vp-c-bg);
  font-size: 13px;
}

.filename {
  font-family: var(--vp-font-family-mono);
  font-size: 12.5px;
  color: var(--vp-c-text-2);
}

.label {
  font-weight: 500;
  color: var(--vp-c-text-2);
}

.actions {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

.status {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  padding-right: 8px;
  font-size: 12px;
  color: var(--vp-c-text-3);
}

.status.ok .pl-icon {
  color: var(--vp-c-green-1);
}

.status.error .pl-icon {
  color: var(--vp-c-danger-1);
}

/* The editor and its first paint share these metrics. */
.editor {
  --pl-code-size: 13.5px;
  --pl-code-line: 1.65;
  overflow: auto;
}

.fallback {
  padding: 12px 0;
}

.editor :deep(.cm-editor) {
  height: 100%;
  background: transparent;
  color: var(--vp-c-text-1);
  font-size: var(--pl-code-size);
}

.editor :deep(.cm-editor.cm-focused) {
  outline: none;
}

.editor :deep(.cm-scroller) {
  font-family: var(--vp-font-family-mono);
  line-height: var(--pl-code-line);
}

.editor :deep(.cm-content) {
  padding: 12px 0;
}

.editor :deep(.cm-line) {
  padding: 0 16px 0 4px;
}

.editor :deep(.cm-gutters) {
  background: transparent;
  border: none;
  color: var(--vp-c-text-3);
}

.editor :deep(.cm-lineNumbers .cm-gutterElement) {
  min-width: 2ch;
  box-sizing: content-box;
  padding: 0 12px 0 16px;
}

.editor :deep(.cm-activeLine) {
  background: color-mix(in srgb, var(--vp-c-text-1) 4%, transparent);
}

.editor :deep(.cm-activeLineGutter) {
  background: transparent;
  color: var(--vp-c-text-1);
}

.editor :deep(.cm-cursor) {
  border-left: 2px solid var(--vp-c-brand-1);
}

.editor :deep(.cm-selectionBackground),
.editor :deep(.cm-focused .cm-selectionBackground) {
  background: rgba(127, 82, 255, 0.24) !important;
}

.editor :deep(.cm-matchingBracket) {
  background: rgba(127, 82, 255, 0.2);
  outline: none;
}

.output {
  min-height: 84px;
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
  color: var(--vp-c-text-1);
}

.stdout + .stderr {
  margin-top: 8px;
}

.stderr {
  color: var(--vp-c-danger-1) !important;
}

.hint {
  margin: 0;
  font-size: 13px;
  line-height: 1.6;
  color: var(--vp-c-text-3);
}

kbd {
  display: inline-block;
  padding: 0 5px;
  border: 1px solid var(--vp-c-border);
  border-radius: 4px;
  font-family: var(--vp-font-family-base);
  font-size: 11px;
  line-height: 17px;
  color: var(--vp-c-text-2);
  background-color: var(--vp-c-bg);
}
</style>
