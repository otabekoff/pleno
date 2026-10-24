<script setup lang="ts">
import { computed } from 'vue'
import { useData, withBase } from 'vitepress'
import { useLayout } from 'vitepress/theme'
import PlIcon, { type IconName } from './PlIcon.vue'

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
      { text: 'Blog', href: '/blog/' },
      { text: 'Roadmap to 1.0', href: 'https://github.com/otabekoff/pleno/blob/main/tasks/v1.md' },
      { text: 'Changelog', href: 'https://github.com/otabekoff/pleno/blob/main/CHANGELOG.md' },
      { text: 'Releases', href: 'https://github.com/otabekoff/pleno/releases' },
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

// The Telegram and Instagram accounts do not exist yet.
const socials: { label: string; icon: IconName; href: string }[] = [
  { label: 'GitHub', icon: 'github', href: 'https://github.com/otabekoff/pleno' },
  { label: 'Telegram', icon: 'telegram', href: '#' },
  { label: 'Instagram', icon: 'instagram', href: '#' },
]

const href = (link: string) => (link.startsWith('http') || link === '#' ? link : withBase(link))
</script>

<template>
  <footer v-if="show" class="site-footer">
    <div class="inner pl-container">
      <div class="brand">
        <a class="logo" :href="withBase('/')">
          <img :src="withBase('/logo.svg')" alt="" width="24" height="24" />
          <span>Pleno</span>
        </a>
        <p>Code that says what it means.</p>
        <div class="socials">
          <a
            v-for="social in socials"
            :key="social.label"
            class="pl-btn pl-btn-ghost pl-btn-sm pl-btn-icon"
            :href="social.href"
            :aria-label="social.label"
            :title="social.label"
            :target="social.href.startsWith('http') ? '_blank' : undefined"
            rel="noopener"
          >
            <PlIcon :name="social.icon" :size="16" />
          </a>
        </div>
      </div>
      <nav v-for="column in columns" :key="column.title" class="column" :aria-label="column.title">
        <h4>{{ column.title }}</h4>
        <a v-for="link in column.links" :key="link.text" :href="href(link.href)">{{ link.text }}</a>
      </nav>
    </div>
    <div class="bottom pl-container">
      <div class="bottom-inner">
        <p>Released under the MIT License.</p>
        <p>© 2026 Otabek and Pleno contributors</p>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.site-footer {
  margin-top: 96px;
  border-top: 1px solid var(--vp-c-divider);
  background-color: var(--vp-c-bg-alt);
}

.inner {
  display: grid;
  gap: 32px;
  padding-top: 48px;
  padding-bottom: 40px;
}

@media (min-width: 640px) {
  .inner {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .brand {
    grid-column: 1 / -1;
  }
}

@media (min-width: 960px) {
  .inner {
    grid-template-columns: 1.5fr repeat(4, 1fr);
    padding-top: 56px;
    padding-bottom: 48px;
  }

  .brand {
    grid-column: auto;
  }
}

.brand {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
}

.logo {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 17px;
  font-weight: 700;
  letter-spacing: -0.01em;
  color: var(--vp-c-text-1);
}

.brand p {
  margin: 10px 0 0;
  font-size: 14px;
  color: var(--vp-c-text-2);
}

.socials {
  display: flex;
  gap: 4px;
  margin: 16px 0 0 -6px;
}

.column {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 10px;
}

.column h4 {
  margin: 0 0 2px;
  font-size: 13px;
  font-weight: 600;
  color: var(--vp-c-text-1);
}

.column a {
  font-size: 14px;
  color: var(--vp-c-text-2);
  transition: color 0.15s;
}

.column a:hover {
  color: var(--vp-c-text-1);
}

.bottom {
  padding-bottom: 28px;
}

.bottom-inner {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 4px 24px;
  padding-top: 20px;
  border-top: 1px solid var(--vp-c-divider);
  font-size: 13px;
  color: var(--vp-c-text-3);
}

.bottom p {
  margin: 0;
}

@media (min-width: 960px) {
  .bottom {
    padding: 0 48px 32px;
  }
}
</style>
