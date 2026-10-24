<script setup lang="ts">
// The heading of a blog post, from its frontmatter, shown above the article
// (the `doc-before` slot) on pages under /blog/.
import { computed } from 'vue'
import { useData, withBase } from 'vitepress'
import { formatDate } from '../format-date'
import PlIcon from './PlIcon.vue'

const { frontmatter, page } = useData()
const isPost = computed(() => page.value.relativePath.startsWith('blog/') && page.value.relativePath !== 'blog/index.md')
const date = computed(() => {
  const value = frontmatter.value.date
  return value ? new Date(value).toISOString().slice(0, 10) : ''
})
</script>

<template>
  <header v-if="isPost" class="post-header">
    <a class="back" :href="withBase('/blog/')">
      <PlIcon name="chevron-left" :size="14" />
      Blog
    </a>
    <h1>{{ frontmatter.title }}</h1>
    <p class="meta">
      <time v-if="date" :datetime="date">{{ formatDate(date) }}</time>
      <template v-if="frontmatter.author"><span aria-hidden="true">·</span> {{ frontmatter.author }}</template>
    </p>
  </header>
</template>

<style scoped>
.post-header {
  margin-bottom: 32px;
  padding-bottom: 28px;
  border-bottom: 1px solid var(--vp-c-divider);
}

.back {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin-left: -2px;
  font-size: 14px;
  font-weight: 500;
  color: var(--vp-c-text-2);
  transition: color 0.15s;
}

.back:hover {
  color: var(--vp-c-text-1);
}

h1 {
  margin: 16px 0 0;
  font-size: clamp(30px, 4.6vw, 40px);
  font-weight: 700;
  line-height: 1.15;
  letter-spacing: -0.03em;
}

.meta {
  display: flex;
  gap: 8px;
  margin: 14px 0 0;
  font-size: 14px;
  color: var(--vp-c-text-3);
}
</style>
