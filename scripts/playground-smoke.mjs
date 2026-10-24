// Runs a few programs through the WebAssembly build that the docs playground
// uses (built by scripts/build-playground.sh) and checks their output. Some
// bugs only appear on wasm32, where function pointers are table indices.
//
// Usage: node scripts/playground-smoke.mjs
import { readFileSync } from 'node:fs'
import { fileURLToPath, pathToFileURL } from 'node:url'
import path from 'node:path'

const dir = path.join(path.dirname(fileURLToPath(import.meta.url)), '../docs/.vitepress/theme/wasm')
const pleno = await import(pathToFileURL(path.join(dir, 'pleno.js')))
await pleno.default({ module_or_path: readFileSync(path.join(dir, 'pleno_bg.wasm')) })

const cases = [
  ['print("hello", 6 * 7);', true, 'hello 42\n'],
  ['print([1, 2, 3] |> map(fn(x) => x * 2) |> filter(fn(x) => x > 2));', true, '[4, 6]\n'],
  ['print([1, 2, 3] |> reduce(fn(a, b) => a + b, 0), (1..=3).toArray());', true, '6 [1, 2, 3]\n'],
  ['import { JSON } from "std:json"; print(JSON.parse(`{"a": [1]}`).a.length);', true, '1\n'],
  ['print(await Promise.all([spawn(fn() => 1), Promise.resolve(2), 3]));', true, '[1, 2, 3]\n'],
  [
    'import { test, assert } from "std:test"; test("fails", fn() { assert.equal(1, 2); });',
    false,
    null,
  ],
  ['while (true) {}', false, null],
]

let failed = 0
for (const [source, ok, output] of cases) {
  const result = JSON.parse(pleno.run_playground(source))
  const good = result.ok === ok && (output === null || result.output === output)
  if (!good) {
    failed++
    console.error(`FAIL ${source}\n  expected ok=${ok} ${JSON.stringify(output)}\n  got ${JSON.stringify(result)}`)
  }
}
console.log(`${cases.length - failed} of ${cases.length} playground checks passed`)
process.exit(failed ? 1 : 0)
