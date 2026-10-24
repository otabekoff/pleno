<script setup lang="ts">
// Highlighted Pleno code rendered on the server, with the same tokenizer and
// colors as the editor. With `lineNumbers` it matches the editor's layout,
// so CodeRunner shows it until CodeMirror has loaded.
import { computed } from 'vue'
import { highlightLines } from '../editor/pleno-tokens'

const props = defineProps<{ code: string; lineNumbers?: boolean }>()
const lines = computed(() => highlightLines(props.code))
</script>

<template>
  <pre class="pl-code" :class="{ numbered: lineNumbers }"><code><span
    v-for="(line, i) in lines"
    :key="i"
    class="line"
  ><span v-if="lineNumbers" class="number" aria-hidden="true">{{ i + 1 }}</span><span class="content" v-html="line || ' '" /></span></code></pre>
</template>

<style scoped>
.pl-code {
  margin: 0;
  font-family: var(--vp-font-family-mono);
  font-size: var(--pl-code-size, 13px);
  line-height: var(--pl-code-line, 1.65);
  color: var(--vp-c-text-1);
  white-space: pre;
}

.pl-code code {
  display: block;
  font: inherit;
  color: inherit;
}

.line {
  display: block;
}

.numbered .number {
  display: inline-block;
  box-sizing: content-box;
  min-width: 2ch;
  padding: 0 12px 0 16px;
  text-align: right;
  color: var(--vp-c-text-3);
  user-select: none;
}

.numbered .content {
  padding: 0 16px 0 4px;
}
</style>
