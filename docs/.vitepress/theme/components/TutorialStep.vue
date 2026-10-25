<script setup lang="ts">
// One tutorial lesson: the explanation (the page's Markdown, passed as the
// default slot) beside an editor with the lesson's starting code. "Show me"
// replaces the code with the solution, as in the Vue tutorial.
import { computed, ref, watch } from 'vue'
import { useData, useRouter, withBase } from 'vitepress'
import CodeRunner from './CodeRunner.vue'
import PlIcon from './PlIcon.vue'
import PlSelect from './PlSelect.vue'
import { steps } from '../tutorial-steps'

const { frontmatter, page } = useData()
const router = useRouter()
const runner = ref<InstanceType<typeof CodeRunner>>()
const showingSolution = ref(false)

const options = steps.map((s, i) => ({ value: s.slug, label: s.title, hint: String(i + 1) }))
const index = computed(() => steps.findIndex((s) => page.value.relativePath === `tutorial/${s.slug}.md`))
const step = computed(() => steps[index.value])
const prev = computed(() => steps[index.value - 1])
const next = computed(() => steps[index.value + 1])
const code = computed(() => (frontmatter.value.code as string) ?? '')
const solution = computed(() => frontmatter.value.solution as string | undefined)

watch(code, () => (showingSolution.value = false))

function toggleSolution() {
  if (!solution.value) return
  showingSolution.value = !showingSolution.value
  runner.value?.setCode(showingSolution.value ? solution.value : code.value)
}

function go(slug: string) {
  router.go(withBase(`/tutorial/${slug}`))
}
</script>

<template>
  <div class="tutorial pl-container">
    <article class="lesson">
      <header class="lesson-head">
        <PlSelect
          class="step-select"
          :model-value="step?.slug ?? ''"
          :options="options"
          label="Lesson"
          @update:model-value="go"
        />
        <span class="progress-text">{{ index + 1 }} of {{ steps.length }}</span>
      </header>
      <div class="progress" role="progressbar" :aria-valuenow="index + 1" aria-valuemin="1" :aria-valuemax="steps.length">
        <span :style="{ width: `${((index + 1) / steps.length) * 100}%` }" />
      </div>

      <div class="vp-doc lesson-body">
        <slot />
      </div>

      <footer class="lesson-foot">
        <a v-if="prev" class="pl-btn pl-btn-ghost nav" :href="withBase(`/tutorial/${prev.slug}`)" :aria-label="`Previous: ${prev.title}`">
          <PlIcon name="chevron-left" />
          <span class="nav-text">{{ prev.title }}</span>
        </a>
        <span v-else />
        <div class="foot-right">
          <button v-if="solution" type="button" class="pl-btn pl-btn-secondary" @click="toggleSolution">
            <PlIcon :name="showingSolution ? 'reset' : 'eye'" />
            {{ showingSolution ? 'Reset' : 'Show me' }}
          </button>
          <a v-if="next" class="pl-btn pl-btn-primary" :href="withBase(`/tutorial/${next.slug}`)" :aria-label="`Next: ${next.title}`">
            <span class="nav-text">{{ next.title }}</span>
            <PlIcon name="chevron-right" />
          </a>
        </div>
      </footer>
    </article>

    <div class="workspace">
      <CodeRunner ref="runner" :code="code" :filename="`${step?.slug ?? 'lesson'}.pln`" height="var(--editor-height)" />
    </div>
  </div>
</template>

<style scoped>
.tutorial {
  --editor-height: 360px;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 24px;
  padding-top: 20px;
  padding-bottom: 40px;
}

@media (min-width: 1024px) {
  .tutorial {
    /* The page height, less the padding and the editor's header bar. */
    --editor-height: calc(100vh - var(--vp-nav-height) - var(--vp-layout-top-height, 0px) - 94px);
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.15fr);
    gap: 32px;
    height: calc(100vh - var(--vp-nav-height) - var(--vp-layout-top-height, 0px));
    padding-top: 24px;
    padding-bottom: 24px;
  }

  /* The lesson picker stays at the top and the buttons at the bottom; only
     the text between them scrolls. */
  .lesson {
    min-height: 0;
  }

  .lesson-body {
    min-height: 0;
    overflow-y: auto;
    margin-right: -16px;
    padding-right: 16px;
  }

  .lesson-foot {
    margin-top: 0;
  }
}

.lesson {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.lesson-head {
  display: flex;
  align-items: center;
  gap: 12px;
}

.step-select {
  flex: 1;
  max-width: 340px;
}

.progress-text {
  margin-left: auto;
  font-size: 13px;
  color: var(--vp-c-text-3);
  font-variant-numeric: tabular-nums;
}

.progress {
  height: 2px;
  margin: 14px 0 4px;
  border-radius: 2px;
  background-color: var(--vp-c-divider);
  overflow: hidden;
}

.progress span {
  display: block;
  height: 100%;
  background-color: var(--vp-c-brand-3);
  transition: width 0.3s;
}

.lesson-body {
  flex: 1;
  padding-bottom: 24px;
}

.lesson-body :deep(h1) {
  margin-top: 20px;
  font-size: 28px;
}

.lesson-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 8px;
  padding-top: 16px;
  border-top: 1px solid var(--vp-c-divider);
}

.foot-right {
  display: flex;
  gap: 8px;
}

.nav {
  padding-left: 8px;
  min-width: 0;
}

.nav-text {
  overflow: hidden;
  text-overflow: ellipsis;
}

@media (max-width: 640px) {
  .nav-text {
    display: none;
  }

  .nav {
    width: 40px;
    padding: 0;
  }

  .foot-right .pl-btn-primary {
    width: 40px;
    padding: 0;
  }
}

.workspace {
  min-width: 0;
}
</style>
