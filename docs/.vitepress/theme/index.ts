// https://vitepress.dev/guide/custom-theme
import { h } from 'vue'
import type { Theme } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import '@fontsource-variable/jetbrains-mono'
import './styles/vars.css'
import './styles/base.css'
import './styles/controls.css'
import './styles/layout.css'
import './styles/pages.css'
import AnnouncementBar from './components/AnnouncementBar.vue'
import BlogIndex from './components/BlogIndex.vue'
import BlogPostHeader from './components/BlogPostHeader.vue'
import CodeRunner from './components/CodeRunner.vue'
import HomeInstall from './components/HomeInstall.vue'
import PlaygroundPage from './components/PlaygroundPage.vue'
import PlenoHome from './components/PlenoHome.vue'
import SiteFooter from './components/SiteFooter.vue'
import TeamContributors from './components/TeamContributors.vue'
import TutorialStep from './components/TutorialStep.vue'

export default {
  extends: DefaultTheme,
  Layout: () => {
    return h(DefaultTheme.Layout, null, {
      // https://vitepress.dev/guide/extending-default-theme#layout-slots
      'layout-top': () => h(AnnouncementBar),
      'doc-before': () => h(BlogPostHeader),
      'layout-bottom': () => h(SiteFooter),
    })
  },
  enhanceApp({ app }) {
    app.component('BlogIndex', BlogIndex)
    app.component('CodeRunner', CodeRunner)
    app.component('HomeInstall', HomeInstall)
    app.component('PlaygroundPage', PlaygroundPage)
    app.component('PlenoHome', PlenoHome)
    app.component('TeamContributors', TeamContributors)
    app.component('TutorialStep', TutorialStep)
  },
} satisfies Theme
