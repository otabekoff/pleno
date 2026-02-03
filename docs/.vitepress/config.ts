import { defineConfig } from 'vitepress'
import fs from 'fs'
import path from 'path'

const plenoGrammar = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, '../public/pleno.tmLanguage.json'),
    'utf-8'
  )
)

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Pleno",
  description: "Clarity, Safety, Expressiveness",
  base: "/pleno/",
  srcDir: '.',
  ignoreDeadLinks: true,
  themeConfig: {
    search: {
      provider: 'local',
      // options: {
      //   miniSearch: {
      //   }
      // }
    },
    outline: {
      level: "deep",
      label: "On this page"
    },
    logo: '/logo.svg',
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Specification', link: '/specs/' },
      { text: 'Stdlib', link: '/stdlib/' },
      { text: 'Examples', link: '/examples' }
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Guide',
          items: [
            { text: 'Getting Started', link: '/guide/getting-started' },
            { text: 'Contributing', link: '/guide/contributing' },
          ]
        }
      ],
      '/specs/': [
        {
          text: 'Language Specification',
          items: [
            { text: 'Introduction', link: '/specs/' },
            { text: 'Core Syntax', link: '/specs/core-syntax-fundamentals' },
            { text: 'Control Flow', link: '/specs/control-flow' },
            { text: 'Functions', link: '/specs/functions' },
            { text: 'Object-Oriented Programming', link: '/specs/OOP' },
            { text: 'Data Structures', link: '/specs/data-structures' },
            { text: 'Functional Features', link: '/specs/functional-programming' },
            { text: 'Async & Concurrency', link: '/specs/async-concurrency' },
            { text: 'Modules & Imports', link: '/specs/modules-imports' },
            { text: 'Error Handling', link: '/specs/error-handling' },
            { text: 'Generics', link: '/specs/generics' },
            { text: 'Decorators', link: '/specs/decorators-attributes' },
            { text: 'Standard Library', link: '/specs/standard-library' },
            { text: 'Memory & Performance', link: '/specs/memory-performance' },
            { text: 'Security', link: '/specs/security-features' },
            { text: 'Tooling & CLI', link: '/specs/tooling-cli' },
            { text: 'Testing', link: '/specs/testing-framework' },
            { text: 'Package Manager', link: '/specs/package-manager' },
            { text: 'Release', link: '/specs/release' },
          ]
        }
      ],
      '/stdlib/': [
        {
          text: 'Standard Library',
          items: [
            // TODO: This should be auto-generated or listed manually if generator doesn't update config
            // For now, linking to index if generated, or listing known modules
            { text: 'Core', link: '/stdlib/core' },
            { text: 'IO', link: '/stdlib/io' },
            { text: 'HTTP', link: '/stdlib/http' },
            { text: 'JSON', link: '/stdlib/json' },
            { text: 'Math', link: '/stdlib/math' },
            { text: 'Regex', link: '/stdlib/regex' },
            { text: 'Time', link: '/stdlib/time' },
            { text: 'OS', link: '/stdlib/os' },
            { text: 'Log', link: '/stdlib/log' },
            { text: 'Benchmark', link: '/stdlib/benchmark' },
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

    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2026-present Pleno Team'
    }
  },
  markdown: {
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
