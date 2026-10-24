// The blog's posts, newest first, from the frontmatter of docs/blog/*.md.
import { createContentLoader } from 'vitepress'

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
      .map((page) => ({
        title: page.frontmatter.title,
        url: page.url,
        date: new Date(page.frontmatter.date).toISOString().slice(0, 10),
        author: page.frontmatter.author ?? '',
        description: page.frontmatter.description ?? '',
      }))
      .sort((a, b) => b.date.localeCompare(a.date))
  },
})
