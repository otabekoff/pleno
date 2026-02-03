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
