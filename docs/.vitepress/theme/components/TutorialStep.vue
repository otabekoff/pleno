<script setup lang="ts">
// One tutorial lesson: the explanation (the page's Markdown, passed as the
// default slot) beside an editor with the lesson's starting code. "Show me"
// replaces the code with the solution, as in the Vue tutorial.
import { computed, ref, watch } from 'vue'
import { useData, useRouter, withBase } from 'vitepress'
import CodeRunner from './CodeRunner.vue'
import { steps } from '../tutorial-steps'

const { frontmatter, page } = useData()
const router = useRouter()
const runner = ref<InstanceType<typeof CodeRunner>>()
const showingSolution = ref(false)

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
  <div class="tutorial">
    <article class="lesson">
      <header class="lesson-head">
        <select
          class="step-select"
          aria-label="Choose a lesson"
          @change="go(($event.target as HTMLSelectElement).value)"
        >
          <option v-for="(s, i) in steps" :key="s.slug" :value="s.slug" :selected="s.slug === step?.slug">{{ i + 1 }}. {{ s.title }}</option>
        </select>
        <span class="progress">{{ index + 1 }} / {{ steps.length }}</span>
      </header>
      <div class="progress-bar"><span :style="{ width: `${((index + 1) / steps.length) * 100}%` }" /></div>

      <div class="vp-doc lesson-body">
        <slot />
      </div>

      <footer class="lesson-foot">
        <a v-if="prev" class="nav prev" :href="withBase(`/tutorial/${prev.slug}`)">← {{ prev.title }}</a>
        <span v-else />
        <button v-if="solution" class="show-me" @click="toggleSolution">
          {{ showingSolution ? 'Reset' : 'Show me' }}
        </button>
        <a v-if="next" class="nav next" :href="withBase(`/tutorial/${next.slug}`)">{{ next.title }} →</a>
      </footer>
    </article>

    <div class="workspace">
      <CodeRunner
        ref="runner"
        :code="code"
        :filename="`${step?.slug ?? 'lesson'}.pln`"
        height="calc(100vh - var(--vp-nav-height) - var(--vp-layout-top-height, 0px) - 300px)"
      />
    </div>
  </div>
</template>

<style scoped>
.tutorial {
  display: grid;
  gap: 24px;
  max-width: 1440px;
  margin: 0 auto;
  padding: 24px;
}

@media (min-width: 1024px) {
  .tutorial {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.15fr);
    height: calc(100vh - var(--vp-nav-height) - var(--vp-layout-top-height, 0px));
    padding: 24px 32px;
  }

  .lesson {
    overflow-y: auto;
    padding-right: 12px;
  }

  .workspace {
    overflow-y: auto;
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
  min-width: 0;
  padding: 8px 14px;
  border: 1px solid var(--vp-c-border);
  border-radius: 999px;
  background: var(--vp-c-bg-soft);
  color: var(--vp-c-text-1);
  font-family: var(--pl-font-display);
  font-size: 15px;
  font-weight: 600;
}

.progress {
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
  color: var(--vp-c-text-3);
}

.progress-bar {
  height: 3px;
  margin: 14px 0 8px;
  border-radius: 3px;
  background: var(--vp-c-divider);
  overflow: hidden;
}

.progress-bar span {
  display: block;
  height: 100%;
  background: var(--pl-gradient);
  transition: width 0.3s;
}

.lesson-body {
  flex: 1;
}

.lesson-body :deep(h1) {
  font-size: 28px;
  margin-top: 16px;
}

.lesson-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 32px;
  padding-top: 16px;
  border-top: 1px solid var(--vp-c-divider);
}

.nav {
  font-size: 14px;
  font-weight: 600;
  color: var(--vp-c-brand-1);
}

.nav:hover {
  text-decoration: underline;
  text-underline-offset: 3px;
}

.show-me {
  padding: 7px 18px;
  border-radius: 999px;
  font-size: 14px;
  font-weight: 600;
  color: #fff;
  background: var(--pl-gradient);
}

.show-me:hover {
  filter: brightness(1.08);
}

.workspace {
  min-width: 0;
}
</style>
