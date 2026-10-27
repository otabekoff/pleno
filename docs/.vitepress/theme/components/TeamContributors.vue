<script setup lang="ts">
// The contributors grid on docs/team.md, from team.data.ts (fetched at build time),
// drawn with VitePress's own team cards so it matches the theme in light and dark.
import { computed } from 'vue'
import { VPTeamMembers } from 'vitepress/theme'
import { data } from '../team.data'

const plural = (n: number) => (n === 1 ? '1 commit' : `${n} commits`)

const members = computed(() =>
  data.contributors.map((person) => ({
    avatar: person.avatar,
    name: person.login,
    title: person.contributions ? plural(person.contributions) : 'Contributor',
    links: [{ icon: 'github', link: person.url }],
  })),
)
</script>

<template>
  <VPTeamMembers size="small" :members="members" />
  <p v-if="!data.fetched" class="note">
    This list could not be loaded from GitHub when the site was built; see the
    <a href="https://github.com/otabekoff/pleno/graphs/contributors" target="_blank" rel="noopener">contributors on GitHub</a>.
  </p>
</template>

<style scoped>
.note {
  font-size: 14px;
  color: var(--vp-c-text-2);
}
</style>
