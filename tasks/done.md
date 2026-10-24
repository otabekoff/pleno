History & motivation. This becomes your project diary. Very powerful psychologically.


# Done

## 2026-10-24
- [x] Compound assignment operators (`+=`, `-=`, `*=`, `/=`, `%=`)
- [x] Fix DCE dropping variables used only in `match`, assignments and other uncovered nodes
- [x] Fix `?.` short-circuiting past `??` and enclosing calls
- [x] Fix GC use-after-free for objects held only in registers
- [x] Generic classes/interfaces: NaN-box bound types; unbound type params accept any type
- [x] Fix class field type checks reading a register instead of the type constant
- [x] Promise rejection: `reject`, `.catch`, `.finally`, `Promise.resolve/reject`, `await` throws, async throws reject
- [x] Runtime errors report their source location
- [x] Enum instance methods, explicit enum values (`A = 1`), structural variant equality
- [x] `extend` on built-in types (`String`, `Int`, `Array`, `Map`, ...)
- [x] Custom exceptions (`class E extends Error`), inherited constructors, multi-level `super()`
- [x] Partial application placeholders (`f(_, 2)`)
- [x] Function and class decorators; metadata-only annotations no longer crash
- [x] Type annotations: `T?`, unions, `T[]`, `Map<K, V>`, function types, class names
- [x] `if` expressions with block branches, keyword property/map keys (`p.catch`), `let deep`
- [x] Tuples: `.0` and named fields; `File.open(path, "r")`; `Math`/`Reflect` namespaces; universal `toString()`
- [x] CI: clippy clean; nightly ARM64 cross-compiler; release `.deb` upload; installer checksum checks
- [x] HTTP server: `res.status/send/json/header`, `:param` routes, query, headers, bodies, correct framing
- [x] Debugger: waits for `configurationDone`, real line stepping, breakpoints in the main file, output events; VS Code uses `pleno-dap`
- [x] WASM builds for `wasm32-unknown-unknown`; local package registry; `pleno.toml` package imports
- [x] Macro splice, `const` initializer check, unhandled rejection warnings, capture analysis for `try`/`throw`/`do-while`
- [x] Correct line numbers inside functions; operand types in operator errors
- [x] `scripts/run_pln_tests.sh` runs every `.pln` script against declared expectations in CI

## 2026-01-23
- [x] Setup project structure
- [x] Add basic CLI

## 2026-01-20
- [x] Initial parser
