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
  srcDir: '.',
  ignoreDeadLinks: true,
  themeConfig: {
    logo: '/logo.png',
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Guide', link: '/specs/index' },
      { text: 'Reference', link: '/specs/core-syntax-fundamentals' },
      { text: 'Examples', link: '/specs/example-programs' }
    ],

    sidebar: [
      {
        text: 'Getting Started',
        items: [
          { text: 'Introduction', link: '/specs/index' },
          { text: 'Bytecode VM Design', link: '/specs/bytecode_vm' },
        ]
      },
      {
        text: 'Language Reference',
        items: [
          { text: 'Core Syntax', link: '/specs/core-syntax-fundamentals' },
          { text: 'Control Flow', link: '/specs/control-flow' },
          { text: 'Functions', link: '/specs/functions' },
          { text: 'Object-Oriented Programming', link: '/specs/OOP' },
          { text: 'Data Structures', link: '/specs/data-structures' },
          { text: 'Functional Features', link: '/specs/functional-programming' },
        ]
      },
      {
        text: 'Advanced',
        items: [
          { text: 'Async & Concurrency', link: '/specs/async-concurrency' },
          { text: 'Error Handling', link: '/specs/error-handling' },
          { text: 'Generics', link: '/specs/generics' },
          { text: 'Decorators', link: '/specs/decorators-attributes' },
          { text: 'Memory & Performance', link: '/specs/memory-performance' },
        ]
      },
      {
        text: 'Standard Library',
        items: [
          { text: 'Overview', link: '/specs/standard-library' },
        ]
      },
      {
        text: 'Tooling',
        items: [
          { text: 'CLI', link: '/specs/tooling-cli' },
          { text: 'Testing', link: '/specs/testing-framework' },
          { text: 'Package Manager', link: '/specs/package-manager' },
        ]
      },
      {
        text: 'Project',
        items: [
          { text: 'Release Process', link: '/specs/release' },
          { text: 'Conclusion', link: '/specs/conclusion' },
        ]
      }
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/plang/pleno' }
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
