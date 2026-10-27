<script setup lang="ts">
// The contributors grid on docs/team.md, from team.data.ts (fetched at build time).
import { data } from '../team.data'

const plural = (n: number) => (n === 1 ? '1 commit' : `${n} commits`)
</script>

<template>
  <ul class="contributors">
    <li v-for="person in data.contributors" :key="person.login">
      <a :href="person.url" target="_blank" rel="noopener">
        <img :src="person.avatar" alt="" width="48" height="48" loading="lazy" />
        <span class="login">{{ person.login }}</span>
        <span v-if="person.contributions" class="count">{{ plural(person.contributions) }}</span>
      </a>
    </li>
  </ul>
  <p v-if="!data.fetched" class="note">
    This list could not be loaded from GitHub when the site was built.
  </p>
</template>

<style scoped>
.contributors {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 12px;
  margin: 16px 0;
  padding: 0;
  list-style: none;
}

.contributors li {
  margin: 0;
}

.contributors a {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 14px 16px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  background-color: var(--vp-c-bg-soft);
  text-decoration: none;
  color: var(--vp-c-text-1);
  transition: border-color 0.15s;
}

.contributors a:hover {
  border-color: var(--vp-c-brand-1);
}

.contributors img {
  margin-bottom: 8px;
  border-radius: 50%;
}

.login {
  font-weight: 600;
  font-size: 14px;
}

.count {
  font-size: 13px;
  color: var(--vp-c-text-2);
}

.note {
  font-size: 14px;
  color: var(--vp-c-text-2);
}
</style>
