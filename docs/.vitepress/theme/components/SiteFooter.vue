<script setup lang="ts">
import { computed } from 'vue'
import { useData, withBase } from 'vitepress'
import { useLayout } from 'vitepress/theme'

// Shown on pages without a sidebar, unless the page sets `footer: false`.
const { frontmatter } = useData()
const { hasSidebar } = useLayout()
const show = computed(() => !hasSidebar.value && frontmatter.value.footer !== false)

const columns = [
  {
    title: 'Learn',
    links: [
      { text: 'Tutorial', href: '/tutorial/' },
      { text: 'Guide', href: '/guide/introduction' },
      { text: 'Playground', href: '/playground' },
      { text: 'Examples', href: '/examples' },
    ],
  },
  {
    title: 'Reference',
    links: [
      { text: 'Language', href: '/specs/' },
      { text: 'Standard library', href: '/stdlib/' },
      { text: 'CLI and tools', href: '/specs/tooling-cli' },
    ],
  },
  {
    title: 'Project',
    links: [
      { text: 'GitHub', href: 'https://github.com/otabekoff/pleno' },
      { text: 'Roadmap to 1.0', href: 'https://github.com/otabekoff/pleno/blob/main/tasks/v1.md' },
      { text: 'Changelog', href: 'https://github.com/otabekoff/pleno/blob/main/CHANGELOG.md' },
      { text: 'Blog', href: '/blog/' },
    ],
  },
  {
    title: 'Community',
    links: [
      { text: 'Contributing', href: '/guide/contributing' },
      { text: 'Report an issue', href: 'https://github.com/otabekoff/pleno/issues' },
      { text: 'Code signing policy', href: '/code-signing-policy' },
    ],
  },
]

const href = (link: string) => (link.startsWith('http') ? link : withBase(link))
</script>

<template>
  <footer v-if="show" class="site-footer">
    <div class="inner">
      <div class="brand">
        <img :src="withBase('/logo.svg')" alt="" width="28" height="28" />
        <span class="name">Pleno</span>
        <p>Code that says what it means.</p>
      </div>
      <nav v-for="column in columns" :key="column.title" class="column" :aria-label="column.title">
        <h4>{{ column.title }}</h4>
        <a v-for="link in column.links" :key="link.text" :href="href(link.href)">{{ link.text }}</a>
      </nav>
    </div>
    <p class="legal">Released under the MIT License. © 2026-present Otabek and Pleno contributors.</p>
  </footer>
</template>

<style scoped>
.site-footer {
  margin-top: 80px;
  border-top: 1px solid var(--vp-c-divider);
  background: color-mix(in srgb, var(--vp-c-bg-alt) 70%, transparent);
}

.inner {
  display: grid;
  gap: 32px;
  max-width: 1200px;
  margin: 0 auto;
  padding: 48px 24px 24px;
}

@media (min-width: 768px) {
  .inner {
    grid-template-columns: 1.4fr repeat(4, 1fr);
    padding: 56px 48px 24px;
  }
}

.brand {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.name {
  font-family: var(--pl-font-display);
  font-size: 20px;
  font-weight: 700;
}

.brand p {
  flex-basis: 100%;
  margin: 4px 0 0;
  font-size: 14px;
  color: var(--vp-c-text-2);
}

.column {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.column h4 {
  margin: 0 0 4px;
  font-family: var(--pl-font-display);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--vp-c-text-3);
}

.column a {
  font-size: 14px;
  color: var(--vp-c-text-2);
  transition: color 0.2s;
}

.column a:hover {
  color: var(--vp-c-brand-1);
}

.legal {
  max-width: 1200px;
  margin: 0 auto;
  padding: 16px 24px 32px;
  font-size: 13px;
  color: var(--vp-c-text-3);
}

@media (min-width: 768px) {
  .legal {
    padding: 16px 48px 32px;
  }
}
</style>
