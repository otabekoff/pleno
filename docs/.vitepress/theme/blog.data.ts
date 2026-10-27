// The blog's posts, newest first, from the frontmatter of docs/blog/*.md.
import { createContentLoader } from 'vitepress'
import { postDay } from './format-date'

export interface Post {
  title: string
  url: string
  date: string
  author: string
  description: string
}

declare const data: Post[]
export { data }

export default createContentLoader('blog/*.md', {
  transform(pages): Post[] {
    return pages
      .filter((page) => page.frontmatter.date)
      // Newest first by the full date and time; posts with the same time
      // keep a fixed order by URL.
      .sort(
        (a, b) =>
          new Date(b.frontmatter.date).getTime() - new Date(a.frontmatter.date).getTime() ||
          a.url.localeCompare(b.url),
      )
      .map((page) => ({
        title: page.frontmatter.title,
        url: page.url,
        date: postDay(page.frontmatter.date),
        author: page.frontmatter.author ?? '',
        description: page.frontmatter.description ?? '',
      }))
  },
})
