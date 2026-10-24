import { defineConfig } from 'vitepress'
import fs from 'fs'
import path from 'path'

const plenoGrammar = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, '../public/pleno.tmLanguage.json'),
    'utf-8'
  )
)

const BASE = '/'

// Modules the browser build cannot use; examples importing them stay offline.
const OFFLINE_MODULES = /from\s+"std:(io|path|os|process|http|net|db|dotenv)"/

// Pleno code blocks get an "Open in playground" link carrying their source.
// ```pleno norun marks a fragment that cannot run by itself (see
// scripts/check-doc-examples.py), and gets no link.
function playgroundLinks(md: any) {
  const fence = md.renderer.rules.fence
  md.renderer.rules.fence = (tokens: any[], idx: number, ...rest: any[]) => {
    const token = tokens[idx]
    const words = token.info.trim().split(/\s+/)
    const norun = words.includes('norun')
    if (norun) token.info = words.filter((w: string) => w !== 'norun').join(' ')
    const html: string = fence(tokens, idx, ...rest)
    const lang = token.info.trim().split(/[\s{:\[]/)[0]
    if (lang !== 'pleno' && lang !== 'pln') return html
    if (norun || OFFLINE_MODULES.test(token.content)) return html
    const code = Buffer.from(token.content, 'utf-8').toString('base64url')
    const link = `<a class="pl-run-link" href="${BASE}playground#code=${code}">Open in playground</a>`
    return html.replace(/<\/div>\s*$/, `${link}</div>`)
  }
}

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Pleno",
  description: "A readable language with optional type annotations, async built in, and the tools to go with it.",
  base: BASE,
  srcDir: '.',
  // Broken internal links fail the build.
  ignoreDeadLinks: [/^https?:\/\/localhost/],
  lastUpdated: true,
  appearance: 'dark',
  head: [['link', { rel: 'icon', href: `${BASE}logo.svg` }]],
  themeConfig: {
    search: {
      provider: 'local',
    },
    outline: {
      level: "deep",
      label: "On this page"
    },
    logo: '/logo.svg',
    nav: [
      { text: 'Tutorial', link: '/tutorial/', activeMatch: '/tutorial/' },
      { text: 'Guide', link: '/guide/introduction', activeMatch: '/guide/' },
      {
        text: 'Reference',
        activeMatch: '/(specs|stdlib)/',
        items: [
          { text: 'Language', link: '/specs/' },
          { text: 'Standard library', link: '/stdlib/' },
          { text: 'CLI and tools', link: '/specs/tooling-cli' },
          { text: 'Examples', link: '/examples' },
        ]
      },
      { text: 'Playground', link: '/playground' },
      { text: 'Blog', link: '/blog/', activeMatch: '/blog/' },
      {
        text: 'v0.1',
        items: [
          {
            items: [
              { text: 'Changelog', link: 'https://github.com/otabekoff/pleno/blob/main/CHANGELOG.md' },
              { text: 'Releases', link: 'https://github.com/otabekoff/pleno/releases' },
              { text: 'Roadmap to 1.0', link: 'https://github.com/otabekoff/pleno/blob/main/tasks/v1.md' },
            ]
          },
          {
            items: [
              { text: 'Showcase', link: '/showcase' },
              { text: 'Community', link: '/community' },
              { text: 'Contributing', link: '/guide/contributing' },
              { text: 'Code signing policy', link: '/code-signing-policy' },
            ]
          }
        ]
      }
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Introduction',
          items: [
            { text: 'Why Pleno', link: '/guide/introduction' },
            { text: 'Installation', link: '/guide/installation' },
            { text: 'Getting started', link: '/guide/getting-started' },
          ]
        },
        {
          text: 'Working with Pleno',
          items: [
            { text: 'Editor setup', link: '/guide/editor' },
            { text: 'Packages', link: '/guide/packages' },
            { text: 'Testing', link: '/guide/testing' },
          ]
        },
        {
          text: 'Project',
          items: [
            { text: 'Contributing', link: '/guide/contributing' },
          ]
        }
      ],
      '/specs/': [
        {
          text: 'Basics',
          items: [
            { text: 'Overview', link: '/specs/' },
            { text: 'Syntax and types', link: '/specs/core-syntax-fundamentals' },
            { text: 'Control flow', link: '/specs/control-flow' },
            { text: 'Functions', link: '/specs/functions' },
            { text: 'Data structures', link: '/specs/data-structures' },
            { text: 'Error handling', link: '/specs/error-handling' },
            { text: 'Modules and imports', link: '/specs/modules-imports' },
          ]
        },
        {
          text: 'Types and objects',
          items: [
            { text: 'Classes and interfaces', link: '/specs/OOP' },
            { text: 'Generics', link: '/specs/generics' },
            { text: 'Decorators', link: '/specs/decorators-attributes' },
          ]
        },
        {
          text: 'Beyond the basics',
          items: [
            { text: 'Functional programming', link: '/specs/functional-programming' },
            { text: 'Async and concurrency', link: '/specs/async-concurrency' },
            { text: 'Advanced features', link: '/specs/advanced-features' },
            { text: 'Standard library tour', link: '/specs/standard-library' },
            { text: 'Example programs', link: '/specs/example-programs' },
          ]
        },
        {
          text: 'Tools',
          items: [
            { text: 'CLI and tools', link: '/specs/tooling-cli' },
            { text: 'Testing framework', link: '/specs/testing-framework' },
            { text: 'Package manager', link: '/specs/package-manager' },
          ]
        },
        {
          text: 'Runtime and internals',
          collapsed: true,
          items: [
            { text: 'Memory and performance', link: '/specs/memory-performance' },
            { text: 'Performance features', link: '/specs/performance-features' },
            { text: 'Security', link: '/specs/security-features' },
            { text: 'Bytecode VM', link: '/specs/bytecode_vm' },
            { text: 'Distribution plan', link: '/specs/distribution' },
            { text: 'Release plan', link: '/specs/release' },
          ]
        }
      ],
      '/stdlib/': [
        {
          text: 'Core',
          items: [
            { text: 'Overview', link: '/stdlib/' },
            { text: 'Core Types', link: '/stdlib/core' },
            { text: 'Math', link: '/stdlib/math' },
            { text: 'Random', link: '/stdlib/random' },
            { text: 'Time', link: '/stdlib/time' },
          ]
        },
        {
          text: 'I/O & System',
          items: [
            { text: 'IO', link: '/stdlib/io' },
            { text: 'Path', link: '/stdlib/path' },
            { text: 'OS', link: '/stdlib/os' },
            { text: 'Process', link: '/stdlib/process' },
            { text: 'Dotenv', link: '/stdlib/dotenv' },
          ]
        },
        {
          text: 'Network',
          items: [
            { text: 'HTTP', link: '/stdlib/http' },
            { text: 'Net', link: '/stdlib/net' },
            { text: 'URL', link: '/stdlib/url' },
          ]
        },
        {
          text: 'Data Formats',
          items: [
            { text: 'JSON', link: '/stdlib/json' },
            { text: 'YAML', link: '/stdlib/yaml' },
            { text: 'TOML', link: '/stdlib/toml' },
            { text: 'XML', link: '/stdlib/xml' },
            { text: 'CSV', link: '/stdlib/csv' },
            { text: 'Base64', link: '/stdlib/base64' },
          ]
        },
        {
          text: 'Async & Concurrency',
          items: [
            { text: 'Promise', link: '/stdlib/promise' },
            { text: 'Concurrency', link: '/stdlib/concurrency' },
          ]
        },
        {
          text: 'Security & Crypto',
          items: [
            { text: 'Crypto', link: '/stdlib/crypto' },
            { text: 'UUID', link: '/stdlib/uuid' },
          ]
        },
        {
          text: 'Testing & Debugging',
          items: [
            { text: 'Test', link: '/stdlib/test' },
            { text: 'Log', link: '/stdlib/log' },
            { text: 'Benchmark', link: '/stdlib/benchmark' },
          ]
        },
        {
          text: 'Advanced',
          items: [
            { text: 'Reflect', link: '/stdlib/reflect' },
            { text: 'Regex', link: '/stdlib/regex' },
          ]
        }
      ],
      '/blog/': [
        {
          text: 'Blog',
          items: [
            { text: 'All Posts', link: '/blog/' },
            { text: 'The road to 1.0', link: '/blog/toward-1-0' },
            { text: 'Announcing Phase 5', link: '/blog/announcing-phase-5' },
          ]
        }
      ],
      '/examples': [
        {
          text: 'Examples',
          items: [
            { text: 'Overview', link: '/examples' }
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/otabekoff/pleno' }
    ],

    editLink: {
      pattern: 'https://github.com/otabekoff/pleno/edit/main/docs/:path',
      text: 'Edit this page on GitHub'
    }
  },
  markdown: {
    config: playgroundLinks,
    theme: {
      light: 'light-plus',
      dark: 'dark-plus'
    },
    languages: [
      {
        ...plenoGrammar,
        name: 'pleno',
        id: 'pleno',

        aliases: ['pln']
      }
    ]
  }
})
