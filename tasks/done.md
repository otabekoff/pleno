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
- [x] Code signing: SignPath Foundation requirements (policy page, uninstall scripts, Windows version metadata)
- [x] Memory safety audit (`notes/MEMORY_SAFETY_AUDIT.md`): string refcount leak, dangling range pointer, 47-bit integer wraparound fixed
- [x] Infinity and NaN are real floats (they decoded as `null` or garbage before)
- [x] Arithmetic: division and modulo by zero are errors for every Int/Float mix; `-Int.MIN` and `Int.MIN % -1` report overflow instead of panicking
- [x] Uncaught exceptions report their message (`MyError: bad input`), not the error map
- [x] Generics variance (`out T`, `in T`) enforced; ownership hints (`move`, `&`, `&mut`) before the name or the type
- [x] `async fn(...) {}` lambdas; async HTTP handlers; `std:crypto` hashes, HMAC, AES and JWT
- [x] Property-based tests for opcodes, arithmetic, strings, arrays and full-range integers
- [x] Constant folding matches runtime semantics (Int / Int is a Float, checked overflow)
- [x] Formatter round-trips every test and example and is idempotent: string escapes, floats, interface/abstract members, operators, accessors, parameter markers, lambdas, `match`, one-element tuples, doc comments, blank lines and member order are kept
- [x] `\$` escapes `${` in strings
- [x] LSP, REPL and DAP run on a 64 MB stack (deeply nested code crashed the language server)
- [x] LSP: UTF-16 positions, CRLF, file URIs with spaces; "Prefix with `_`" quick fix
- [x] `vm_errors.rs` covers bytecode and corrupted-chunk errors; empty tests removed
- [x] Specs document `std:gc`, `std:security` and `Mutex`; no `__native_` globals remain
- [x] Fuzzing (lexer, parser, compiler) and benchmark-regression checks run in CI and pass; root `.vitepress` cache untracked
- [x] Docs checked by running every code block (308): syntax and API drift fixed, unimplemented features marked as planned
- [x] Backtick strings; `assert.true/false/null`; enum methods after `;`; `return`/`throw` in match arms; generic type aliases; `extend <T> Array<T>`
- [x] Type checks: enum values against their enum, generic enums, `Result`/`Option` annotations, contravariant interfaces, aliases inside functions
- [x] A skipped typed `catch` no longer overwrites the error for the next one
- [x] `x |> f(a)` calls `f(x, a)` (and fills a `_` placeholder)
- [x] Channels: `for (x in ch)` waits for values and ends on `close()`; values sent before `close()` are delivered
- [x] Class annotations (`@Route("/users")`) are metadata, not decorators
- [x] Every std module offers a namespace and named imports (`Path`, `HTTP`, `OS`, ...); `std:core` and `std:promise` exist
- [x] `std:log` levels and prefixed loggers; `DateTime.parse/add/subtract/isAfter`, `Duration`; `JSON.stringifyPretty`; `Regex.split`; `Benchmark.measure`; `OS.hostname/uptime`; `Set/Map.isEmpty`, `Range.contains/toArray`, `Array.clone`, function `.name`
- [x] `pleno check` prints `file:line:col` and real JSON with `--json`; `pleno --help` lists every command
- [x] VS Code extension 0.2.0: toolchain install/update, accurate tool detection, no duplicate diagnostics or formatters, fixed snippets and walkthrough

## 2026-01-23
- [x] Setup project structure
- [x] Add basic CLI

## 2026-01-20
- [x] Initial parser
