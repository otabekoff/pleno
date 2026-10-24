<script setup lang="ts">
import { withBase } from 'vitepress'
import CodeRunner from './CodeRunner.vue'
import HomeInstall from './HomeInstall.vue'

const heroCode = `enum Shape {
    Circle(radius),
    Rect(width, height)
}

fn area(shape: Shape): Float {
    match (shape) {
        case Shape.Circle(r) => return 3.14159 * r * r,
        case Shape.Rect(w, h) => return w * h
    }
}

let shapes = [Shape.Circle(2.0), Shape.Rect(3.0, 4.5)];
for (shape in shapes) {
    print(\`\${shape}: \${area(shape)}\`);
}
`

const cards = [
  {
    title: 'Types when you want them',
    text: 'Leave them out, or annotate parameters, fields and results. Annotations are checked when the code runs.',
    code: `fn total(prices: Float[]): Float {
    return prices.reduce(fn(a, b) => a + b, 0.0);
}

let maybe: Int? = null;
let id: Int | String = "user-42";`,
    link: '/specs/core-syntax-fundamentals',
    wide: true,
  },
  {
    title: 'Classes that keep secrets',
    text: 'Private fields are really private, and interfaces are checked.',
    code: `class Counter {
    private count: Int = 0;
    fn next(): Int {
        this.count += 1;
        return this.count;
    }
}`,
    link: '/specs/OOP',
  },
  {
    title: 'Async built in',
    text: 'async and await, spawn, channels and mutexes, on an event loop.',
    code: `import { Channel } from "std:concurrency";

let jobs = Channel(8);
spawn(fn() {
    jobs.send("build");
    jobs.close();
});
for (job in jobs) { print(job); }`,
    link: '/specs/async-concurrency',
  },
  {
    title: 'Pipelines and closures',
    text: 'Data flows left to right; functions are values.',
    code: `let evens = 1..=10
    |> filter(fn(n) => n % 2 == 0)
    |> map(fn(n) => n * n);`,
    link: '/specs/functional-programming',
  },
  {
    title: 'Tests in the box',
    text: 'describe, test and assert from std:test; pleno test runs them.',
    code: `import { test, assert } from "std:test";

test("adds", fn() {
    assert.equal(1 + 1, 2);
});`,
    link: '/guide/testing',
  },
]

const stdlib = ['io', 'path', 'process', 'http', 'json', 'yaml', 'toml', 'csv', 'xml', 'regex', 'time', 'crypto', 'uuid', 'db', 'log', 'test', 'concurrency', 'reflect', 'math', 'random']

const status = [
  {
    label: 'Works today',
    items: ['Gradual types, classes, enums, generics', 'async/await, channels, mutexes', 'Standard library of 25+ modules', 'Formatter, linter, tests, LSP, debugger'],
  },
  {
    label: 'Before 1.0',
    items: ['A frozen language specification', 'Faster execution', 'Destructuring and one error shape', 'Signed releases in package managers'],
  },
  {
    label: 'After 1.0',
    items: ['A JIT compiler', 'Generational garbage collector', 'A public package registry', 'A self-hosted compiler'],
  },
]
</script>

<template>
  <div class="pleno-home">
    <!-- Hero -->
    <section class="hero">
      <div class="hero-text">
        <a class="eyebrow" :href="withBase('/blog/toward-1-0')">
          <span class="dot" /> v0.1 · Read about the road to 1.0 →
        </a>
        <h1 class="pl-display">
          Code that says<br />
          <span class="pl-gradient-text">what it means.</span>
        </h1>
        <p class="lead">
          Pleno is a readable, dynamically typed language with optional checked types,
          classes and pattern matching, async built in, and the tools to go with it.
        </p>
        <div class="actions">
          <a class="btn primary" :href="withBase('/tutorial/')">Start the tutorial</a>
          <a class="btn" :href="withBase('/guide/getting-started')">Read the guide</a>
          <a class="btn ghost" :href="withBase('/playground')">Playground</a>
        </div>
        <HomeInstall />
      </div>
      <div class="hero-code">
        <CodeRunner :code="heroCode" height="390px" filename="shapes.pln" />
      </div>
    </section>

    <!-- Highlights -->
    <section class="section">
      <h2 class="pl-display section-title">The language, at a glance</h2>
      <p class="section-lead">Familiar syntax, with the parts you would expect from a modern language.</p>
      <div class="bento">
        <a
          v-for="card in cards"
          :key="card.title"
          class="card"
          :class="{ wide: card.wide }"
          :href="withBase(card.link)"
        >
          <h3>{{ card.title }}</h3>
          <p>{{ card.text }}</p>
          <pre class="snippet"><code>{{ card.code }}</code></pre>
        </a>
        <a class="card stdlib" :href="withBase('/stdlib/')">
          <h3>A standard library that covers the basics</h3>
          <p>Every module can be imported as a namespace or by name.</p>
          <div class="chips">
            <span v-for="name in stdlib" :key="name" class="chip">std:{{ name }}</span>
          </div>
        </a>
      </div>
    </section>

    <!-- Tools -->
    <section class="section tools">
      <div class="tools-text">
        <h2 class="pl-display section-title">One toolchain</h2>
        <p class="section-lead">
          <code>pleno</code> runs, checks, formats, lints and tests. <code>plm</code> creates
          projects and manages dependencies. <code>pleno-lsp</code> and <code>pleno-dap</code> power
          the VS Code extension, which can install all of them for you.
        </p>
        <a class="btn" :href="withBase('/guide/editor')">Set up your editor →</a>
      </div>
      <div class="terminal" role="img" aria-label="A terminal running plm init and plm test">
        <div class="terminal-bar"><span class="dots"><i /><i /><i /></span><span>~/my-app</span></div>
<pre class="terminal-body"><span class="prompt">$</span> plm init my-app

  <span class="pl-gradient-text bold">Pleno</span>  <span class="dim">created my-app</span>

  <span class="ok">✔</span> pleno.toml
  <span class="ok">✔</span> src/main.pln
  <span class="ok">✔</span> tests/integration_test.pln

<span class="prompt">$</span> plm test
math
  <span class="ok">✓</span> adds numbers
  <span class="ok">✓</span> compares numbers
<span class="ok">PASSED</span>

<span class="prompt">$</span> pleno check src/main.pln
src/main.pln:3:5: <span class="err">error:</span> undefined variable 'prnt'</pre>
      </div>
    </section>

    <!-- Status -->
    <section class="section">
      <h2 class="pl-display section-title">Where Pleno is</h2>
      <p class="section-lead">
        Pleno is at 0.1. It runs real programs today; the
        <a href="https://github.com/otabekoff/pleno/blob/main/tasks/v1.md">plan for 1.0</a>
        lists what is left.
      </p>
      <div class="status">
        <div v-for="(column, i) in status" :key="column.label" class="status-column" :class="`step-${i}`">
          <h3>{{ column.label }}</h3>
          <ul>
            <li v-for="item in column.items" :key="item">{{ item }}</li>
          </ul>
        </div>
      </div>
    </section>

    <!-- Call to action -->
    <section class="section cta">
      <h2 class="pl-display">Learn Pleno in an afternoon.</h2>
      <p>Fifteen short lessons, each with code you can edit and run right in the page.</p>
      <a class="btn primary" :href="withBase('/tutorial/')">Start the tutorial</a>
    </section>
  </div>
</template>

<style scoped>
.pleno-home {
  max-width: 1200px;
  margin: 0 auto;
  padding: 0 24px 48px;
}

@media (min-width: 768px) {
  .pleno-home {
    padding: 0 48px 64px;
  }
}

/* Hero */
.hero {
  display: grid;
  gap: 40px;
  align-items: center;
  padding: 56px 0 24px;
}

@media (min-width: 1024px) {
  .hero {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.05fr);
    padding: 88px 0 40px;
  }
}

.eyebrow {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 4px 12px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 999px;
  font-size: 13px;
  font-weight: 500;
  color: var(--vp-c-text-2);
  background: var(--pl-panel);
  transition: border-color 0.2s, color 0.2s;
}

.eyebrow:hover {
  border-color: var(--vp-c-brand-2);
  color: var(--vp-c-text-1);
}

.eyebrow .dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--pl-gradient);
  box-shadow: 0 0 10px var(--pl-magenta);
}

.hero h1 {
  margin: 20px 0 0;
  font-size: clamp(40px, 6vw, 64px);
  line-height: 1.05;
  font-weight: 700;
}

.lead {
  max-width: 560px;
  margin: 20px 0 0;
  font-size: 18px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 28px;
}

.btn {
  display: inline-flex;
  align-items: center;
  padding: 9px 20px;
  border: 1px solid var(--vp-c-border);
  border-radius: 999px;
  font-size: 15px;
  font-weight: 600;
  color: var(--vp-c-text-1);
  background: var(--vp-c-bg-soft);
  transition: border-color 0.2s, filter 0.2s, transform 0.2s;
}

.btn:hover {
  border-color: var(--vp-c-brand-2);
  transform: translateY(-1px);
}

.btn.primary {
  border-color: transparent;
  color: #fff;
  background: var(--pl-gradient);
  box-shadow: 0 8px 28px -10px var(--pl-magenta);
}

.btn.primary:hover {
  filter: brightness(1.08);
}

.btn.ghost {
  background: transparent;
}

.hero-code {
  position: relative;
}

.hero-code::before {
  content: '';
  position: absolute;
  inset: 8% 4%;
  z-index: -1;
  background: var(--pl-gradient);
  filter: blur(70px);
  opacity: 0.28;
}

/* Sections */
.section {
  padding: 72px 0 0;
}

.section-title {
  margin: 0;
  font-size: clamp(28px, 4vw, 38px);
  line-height: 1.15;
  font-weight: 700;
}

.section-lead {
  max-width: 640px;
  margin: 12px 0 0;
  font-size: 17px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.section-lead a {
  color: var(--vp-c-brand-1);
  text-decoration: underline;
  text-underline-offset: 3px;
}

/* Bento grid */
.bento {
  display: grid;
  gap: 16px;
  margin-top: 32px;
}

@media (min-width: 768px) {
  .bento {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (min-width: 1024px) {
  .bento {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .card.wide {
    grid-column: span 2;
  }

  .card.stdlib {
    grid-column: 1 / -1;
  }
}

.card {
  display: flex;
  flex-direction: column;
  padding: 24px;
  border: 1px solid var(--pl-panel-border);
  border-radius: var(--pl-radius-lg);
  background: var(--pl-panel);
  backdrop-filter: blur(10px);
  color: inherit;
  transition: border-color 0.25s, transform 0.25s;
}

.card:hover {
  border-color: color-mix(in srgb, var(--vp-c-brand-2) 60%, transparent);
  transform: translateY(-3px);
}

.card h3 {
  margin: 0;
  font-family: var(--pl-font-display);
  font-size: 18px;
  font-weight: 600;
}

.card p {
  margin: 8px 0 16px;
  font-size: 14px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.snippet {
  margin: auto 0 0;
  padding: 14px 16px;
  border-radius: var(--pl-radius);
  background: var(--vp-c-bg-alt);
  border: 1px solid var(--vp-c-divider);
  overflow-x: auto;
  font-family: var(--vp-font-family-mono);
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--vp-c-text-1);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: auto;
}

.chip {
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-alt);
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  color: var(--vp-c-text-2);
}

/* Tools */
.tools {
  display: grid;
  gap: 32px;
  align-items: center;
}

@media (min-width: 1024px) {
  .tools {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
  }
}

.tools .btn {
  margin-top: 24px;
}

.tools code {
  font-family: var(--vp-font-family-mono);
  font-size: 0.9em;
  color: var(--vp-c-text-1);
}

.terminal {
  border: 1px solid var(--pl-panel-border);
  border-radius: var(--pl-radius-lg);
  background: #0a0910;
  overflow: hidden;
  box-shadow: 0 20px 60px -30px rgba(127, 82, 255, 0.6);
}

.terminal-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  color: #8f89a3;
}

.dots {
  display: inline-flex;
  gap: 6px;
}

.dots i {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.dots i:nth-child(1) { background: #ef4857; }
.dots i:nth-child(2) { background: #d211ec; }
.dots i:nth-child(3) { background: #7f52ff; }

.terminal-body {
  margin: 0;
  padding: 18px 20px;
  overflow-x: auto;
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
  line-height: 1.65;
  color: #e9e6f2;
}

.terminal-body .prompt { color: #b9a0ff; }
.terminal-body .dim { color: #8f89a3; }
.terminal-body .ok { color: #86efac; }
.terminal-body .err { color: #ff7a85; font-weight: 600; }
.terminal-body .bold { font-weight: 700; }

/* Status */
.status {
  display: grid;
  gap: 16px;
  margin-top: 32px;
}

@media (min-width: 768px) {
  .status {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

.status-column {
  position: relative;
  padding: 24px;
  border: 1px solid var(--pl-panel-border);
  border-radius: var(--pl-radius-lg);
  background: var(--pl-panel);
  overflow: hidden;
}

.status-column::before {
  content: '';
  position: absolute;
  inset: 0 0 auto 0;
  height: 3px;
  background: var(--pl-gradient);
}

.status-column.step-1::before { opacity: 0.6; }
.status-column.step-2::before { opacity: 0.3; }

.status-column h3 {
  margin: 0 0 12px;
  font-family: var(--pl-font-display);
  font-size: 16px;
}

.status-column ul {
  margin: 0;
  padding-left: 18px;
  font-size: 14px;
  line-height: 1.9;
  color: var(--vp-c-text-2);
}

/* Call to action */
.cta {
  text-align: center;
}

.cta h2 {
  margin: 0;
  font-size: clamp(28px, 4vw, 40px);
  font-weight: 700;
}

.cta p {
  margin: 12px 0 28px;
  color: var(--vp-c-text-2);
  font-size: 17px;
}
</style>
