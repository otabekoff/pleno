<script setup lang="ts">
import { withBase } from 'vitepress'
import { data as posts } from '../blog.data'
import { formatDate } from '../format-date'
import PlIcon from './PlIcon.vue'
</script>

<template>
  <div class="blog pl-container">
    <header class="head">
      <h1 class="pl-display">Blog</h1>
      <p>News about Pleno and the work toward 1.0.</p>
    </header>
    <ol class="posts">
      <li v-for="post in posts" :key="post.url">
        <a class="post" :href="withBase(post.url)">
          <time :datetime="post.date">{{ formatDate(post.date) }}</time>
          <h2>{{ post.title }}</h2>
          <p>{{ post.description }}</p>
          <span class="read">
            Read the post
            <PlIcon name="arrow-right" :size="14" />
          </span>
        </a>
      </li>
    </ol>
  </div>
</template>

<style scoped>
.blog {
  padding-top: 64px;
  padding-bottom: 24px;
}

.blog > * {
  max-width: 760px;
}

.head h1 {
  margin: 0;
  font-size: clamp(32px, 5vw, 44px);
  font-weight: 700;
  letter-spacing: -0.03em;
}

.head p {
  margin: 10px 0 0;
  font-size: 17px;
  color: var(--vp-c-text-2);
}

.posts {
  display: grid;
  gap: 12px;
  margin: 40px 0 0;
  padding: 0;
  list-style: none;
}

.post {
  display: block;
  padding: 22px 24px;
  border: 1px solid var(--vp-c-border);
  border-radius: var(--pl-radius-lg);
  color: inherit;
  transition: border-color 0.15s, background-color 0.15s;
}

.post:hover {
  border-color: var(--pl-border-strong);
  background-color: var(--vp-c-bg-alt);
}

time {
  font-size: 13px;
  color: var(--vp-c-text-3);
  font-variant-numeric: tabular-nums;
}

.post h2 {
  margin: 6px 0 0;
  font-size: 20px;
  font-weight: 650;
  line-height: 1.35;
  letter-spacing: -0.015em;
}

.post p {
  margin: 8px 0 0;
  font-size: 15px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.read {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 14px;
  font-size: 14px;
  font-weight: 600;
  color: var(--vp-c-brand-1);
}

.read .pl-icon {
  transition: transform 0.15s;
}

.post:hover .read .pl-icon {
  transform: translateX(2px);
}
</style>
