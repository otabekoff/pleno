// Pleno's contributors, fetched from the GitHub API when the site is built, for docs/team.md.
// Bots (such as Dependabot) are left out. When the request fails (no network, rate limit),
// the build goes on with FALLBACK and the page links to the list on GitHub.
// Set GITHUB_TOKEN to raise the API's rate limit.

export interface Contributor {
  login: string
  url: string
  avatar: string
  contributions: number
}

export interface TeamData {
  contributors: Contributor[]
  fetched: boolean
}

declare const data: TeamData
export { data }

const API = 'https://api.github.com/repos/otabekoff/pleno/contributors?per_page=100'

// The maintainer, so the page is never empty.
const FALLBACK: Contributor[] = [
  {
    login: 'otabekoff',
    url: 'https://github.com/otabekoff',
    avatar: 'https://avatars.githubusercontent.com/otabekoff?s=128',
    contributions: 0,
  },
]

export default {
  async load(): Promise<TeamData> {
    try {
      const headers: Record<string, string> = {
        Accept: 'application/vnd.github+json',
        'User-Agent': 'pleno-docs',
      }
      if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`
      const response = await fetch(API, { headers, signal: AbortSignal.timeout(10_000) })
      if (!response.ok) throw new Error(`GitHub API: ${response.status}`)
      const list = (await response.json()) as any[]
      const contributors = list
        .filter((c) => c.type !== 'Bot' && !String(c.login).endsWith('[bot]'))
        .map((c) => ({
          login: c.login,
          url: c.html_url,
          avatar: `${c.avatar_url}${String(c.avatar_url).includes('?') ? '&' : '?'}s=128`,
          contributions: c.contributions,
        }))
      if (contributors.length === 0) throw new Error('GitHub API: empty list')
      return { contributors, fetched: true }
    } catch (error) {
      console.warn(`team.data.ts: using the fallback list (${(error as Error).message})`)
      return { contributors: FALLBACK, fetched: false }
    }
  },
}
