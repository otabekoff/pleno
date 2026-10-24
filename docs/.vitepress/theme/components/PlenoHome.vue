<script setup lang="ts">
import { withBase } from 'vitepress'
import CodeRunner from './CodeRunner.vue'
import HomeInstall from './HomeInstall.vue'
import PlCode from './PlCode.vue'
import PlIcon from './PlIcon.vue'

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

const features = [
  {
    title: 'Types when you want them',
    text: 'Leave them out, or annotate parameters, fields and results. Annotations are checked when the code runs.',
    code: `fn area(w: Float, h: Float): Float {
    return w * h;
}

let maybe: Int? = null;
let id: Int | String = "user-42";`,
    link: '/specs/core-syntax-fundamentals',
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
    title: 'Enums and pattern matching',
    text: 'Enums carry data, and match takes them apart with guards and bindings.',
    code: `let label = match (status) {
    case Status.Ok => "done",
    case Status.Failed(why) => why,
    case _ => "waiting"
};`,
    link: '/specs/data-structures',
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
    text: 'Data flows left to right, and functions are values.',
    code: `let squares = 1..=10
    |> filter(fn(n) => n % 2 == 0)
    |> map(fn(n) => n * n);`,
    link: '/specs/functional-programming',
  },
  {
    title: 'Tests in the box',
    text: 'describe, test and assert come with the language; pleno test runs them.',
    code: `import { test, assert } from "std:test";

test("adds", fn() {
    assert.equal(1 + 1, 2);
});`,
    link: '/guide/testing',
  },
]

const stdlib = ['io', 'path', 'process', 'http', 'net', 'json', 'yaml', 'toml', 'csv', 'xml', 'regex', 'time', 'crypto', 'uuid', 'db', 'log', 'test', 'concurrency', 'reflect', 'math', 'random']

const status = [
  {
    label: 'Works today',
    tone: 'done',
    items: ['Gradual types, classes, enums, generics', 'async and await, channels, mutexes', 'A standard library of 25 modules', 'Formatter, linter, tests, language server, debugger'],
  },
  {
    label: 'Before 1.0',
    tone: 'next',
    items: ['A frozen language specification', 'Faster execution', 'Destructuring and one error shape', 'Signed releases in package managers'],
  },
  {
    label: 'After 1.0',
    tone: 'later',
    items: ['A JIT compiler', 'A generational garbage collector', 'A public package registry', 'A self-hosted compiler'],
  },
]
</script>

<template>
  <div class="home">
    <section class="hero">
      <div class="hero-text">
        <a class="badge" :href="withBase('/blog/toward-1-0')">
          <span class="badge-tag">v0.2</span>
          <span>The road to Pleno 1.0</span>
          <PlIcon name="chevron-right" :size="14" />
        </a>
        <h1 class="pl-display">
          Code that says<br />
          <span class="pl-gradient-text">what it means.</span>
        </h1>
        <p class="lead">
          Pleno is a readable, dynamically typed language with optional checked types, classes
          and pattern matching, async built in, and the tools to go with it.
        </p>
        <div class="actions">
          <a class="pl-btn pl-btn-primary" :href="withBase('/tutorial/')">
            Start the tutorial
            <PlIcon name="arrow-right" />
          </a>
          <a class="pl-btn pl-btn-secondary" :href="withBase('/guide/getting-started')">Read the guide</a>
          <a class="pl-btn pl-btn-ghost" :href="withBase('/playground')">Open the playground</a>
        </div>
        <HomeInstall class="install" />
      </div>
      <div class="hero-code">
        <CodeRunner :code="heroCode" height="404px" filename="shapes.pln" />
      </div>
    </section>

    <section class="section">
      <header class="section-head">
        <h2 class="pl-display">The language at a glance</h2>
        <p>Familiar syntax, with the parts you would expect from a modern language.</p>
      </header>
      <div class="features">
        <a v-for="feature in features" :key="feature.title" class="feature" :href="withBase(feature.link)">
          <h3>{{ feature.title }}</h3>
          <p>{{ feature.text }}</p>
          <div class="sample"><PlCode :code="feature.code" /></div>
        </a>
      </div>
      <a class="stdlib" :href="withBase('/stdlib/')">
        <div class="stdlib-text">
          <h3>A standard library that covers the basics</h3>
          <p>Files, processes, HTTP, data formats, regular expressions, time, crypto, SQLite and tests.</p>
        </div>
        <ul class="modules" aria-label="Standard library modules">
          <li v-for="name in stdlib" :key="name">std:{{ name }}</li>
        </ul>
      </a>
    </section>

    <section class="section tools">
      <div class="tools-text">
        <h2 class="pl-display">One toolchain</h2>
        <p>
          <code>pleno</code> runs, checks, formats, lints and tests. <code>plm</code> creates
          projects and manages dependencies. <code>pleno-lsp</code> and <code>pleno-dap</code>
          power the VS Code extension, which can install all of them for you.
        </p>
        <a class="pl-link" :href="withBase('/guide/editor')">
          Set up your editor
          <PlIcon name="arrow-right" :size="14" />
        </a>
      </div>
      <div class="terminal" role="img" aria-label="A terminal running plm init, plm test and pleno check">
        <div class="terminal-bar">
          <PlIcon name="terminal" :size="14" />
          <span>~/my-app</span>
        </div>
<pre class="terminal-body"><span class="prompt">$</span> plm init my-app

  <span class="brand">Pleno</span>  <span class="dim">created my-app</span>

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

    <section class="section">
      <header class="section-head">
        <h2 class="pl-display">Where Pleno is</h2>
        <p>
          Pleno is at 0.2. It runs real programs today; the
          <a href="https://github.com/otabekoff/pleno/blob/main/tasks/v1.md">plan for 1.0</a>
          lists what is left.
        </p>
      </header>
      <div class="status">
        <div v-for="column in status" :key="column.label" class="status-column">
          <h3><span class="dot" :class="column.tone" aria-hidden="true" />{{ column.label }}</h3>
          <ul>
            <li v-for="item in column.items" :key="item">{{ item }}</li>
          </ul>
        </div>
      </div>
    </section>

    <section class="section cta">
      <h2 class="pl-display">Learn Pleno in an afternoon</h2>
      <p>Fifteen short lessons, each with code you can edit and run in the page.</p>
      <a class="pl-btn pl-btn-primary" :href="withBase('/tutorial/')">
        Start the tutorial
        <PlIcon name="arrow-right" />
      </a>
    </section>
  </div>
</template>

<style scoped>
.home {
  max-width: 1200px;
  margin: 0 auto;
  padding: 0 24px 32px;
}

@media (min-width: 768px) {
  .home {
    padding: 0 48px 48px;
  }
}

/* Hero */
.hero {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 48px;
  align-items: center;
  padding: 56px 0 16px;
}

@media (min-width: 1024px) {
  .hero {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.05fr);
    padding: 80px 0 24px;
  }
}

.badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 10px 0 4px;
  border: 1px solid var(--vp-c-border);
  border-radius: 999px;
  font-size: 13px;
  font-weight: 500;
  color: var(--vp-c-text-2);
  transition: border-color 0.15s, color 0.15s;
}

.badge:hover {
  border-color: var(--pl-border-strong);
  color: var(--vp-c-text-1);
}

.badge-tag {
  padding: 2px 8px;
  border-radius: 999px;
  background-color: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
  font-size: 12px;
  font-weight: 600;
}

.badge .pl-icon {
  color: var(--vp-c-text-3);
}

.hero h1 {
  margin: 24px 0 0;
  font-size: clamp(40px, 5.6vw, 60px);
  line-height: 1.06;
  font-weight: 700;
  letter-spacing: -0.035em;
}

.lead {
  max-width: 540px;
  margin: 20px 0 0;
  font-size: 18px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 32px;
}

.install {
  margin-top: 32px;
}

/* Sections */
.section {
  padding-top: 96px;
}

.section-head h2,
.tools-text h2,
.cta h2 {
  margin: 0;
  font-size: clamp(26px, 3.4vw, 34px);
  line-height: 1.2;
  font-weight: 700;
}

.section-head p,
.tools-text p,
.cta p {
  max-width: 620px;
  margin: 12px 0 0;
  font-size: 16px;
  line-height: 1.65;
  color: var(--vp-c-text-2);
}

.section-head a {
  color: var(--vp-c-brand-1);
  text-decoration: underline;
  text-underline-offset: 3px;
}

/* Features */
.features {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 16px;
  margin-top: 36px;
}

@media (min-width: 720px) {
  .features {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (min-width: 1024px) {
  .features {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

.feature,
.stdlib {
  display: flex;
  flex-direction: column;
  padding: 20px;
  border: 1px solid var(--vp-c-border);
  border-radius: var(--pl-radius-lg);
  background-color: var(--vp-c-bg);
  color: inherit;
  transition: border-color 0.15s;
}

.feature:hover,
.stdlib:hover {
  border-color: var(--pl-border-strong);
}

.feature h3,
.stdlib h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  letter-spacing: -0.01em;
}

.feature p,
.stdlib p {
  margin: 6px 0 16px;
  font-size: 14px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.sample {
  margin-top: auto;
  padding: 14px;
  overflow-x: auto;
  border: 1px solid var(--vp-c-divider);
  border-radius: var(--pl-radius);
  background-color: var(--vp-code-block-bg);
  --pl-code-size: 12px;
  --pl-code-line: 1.6;
}

.stdlib {
  margin-top: 16px;
  gap: 16px;
}

@media (min-width: 1024px) {
  .stdlib {
    flex-direction: row;
    align-items: center;
    gap: 32px;
  }

  .stdlib-text {
    flex: 0 0 300px;
  }

  .stdlib p {
    margin-bottom: 0;
  }
}

.modules {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.modules li {
  padding: 3px 8px;
  border: 1px solid var(--vp-c-divider);
  border-radius: var(--pl-radius-sm);
  background-color: var(--vp-code-block-bg);
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  color: var(--vp-c-text-2);
}

/* Tools */
.tools {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 40px;
  align-items: center;
}

@media (min-width: 1024px) {
  .tools {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
  }
}

.tools-text code {
  font-family: var(--vp-font-family-mono);
  font-size: 0.88em;
  color: var(--vp-c-text-1);
}

.tools-text .pl-link {
  margin-top: 20px;
}

.terminal {
  overflow: hidden;
  border: 1px solid var(--vp-c-border);
  border-radius: var(--pl-radius-lg);
  background-color: #111114;
}

.terminal-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 40px;
  padding: 0 16px;
  border-bottom: 1px solid #24242b;
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  color: #8b8b96;
}

.terminal-body {
  margin: 0;
  padding: 16px 20px;
  overflow-x: auto;
  font-family: var(--vp-font-family-mono);
  font-size: 13px;
  line-height: 1.65;
  color: #e6e6eb;
}

.terminal-body .prompt { color: #8b8b96; }
.terminal-body .dim { color: #8b8b96; }
.terminal-body .brand { color: #ab92ff; font-weight: 700; }
.terminal-body .ok { color: #7ee2a8; }
.terminal-body .err { color: #ff7b72; font-weight: 600; }

/* Status */
.status {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 16px;
  margin-top: 36px;
}

@media (min-width: 768px) {
  .status {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

.status-column {
  padding: 20px;
  border: 1px solid var(--vp-c-border);
  border-radius: var(--pl-radius-lg);
}

.status-column h3 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 12px;
  font-size: 15px;
  font-weight: 600;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.dot.done { background-color: var(--vp-c-green-2); }
.dot.next { background-color: var(--vp-c-yellow-2); }
.dot.later { background-color: var(--vp-c-gray-1); }

.status-column ul {
  margin: 0;
  padding-left: 18px;
  list-style: disc;
  font-size: 14px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.status-column li::marker {
  color: var(--vp-c-text-3);
}

.status-column li + li {
  margin-top: 6px;
}

/* Call to action */
.cta {
  text-align: center;
}

.cta p {
  margin: 12px auto 28px;
}
</style>
