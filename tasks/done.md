# Done

What was finished, newest first. Open work is in `v1.md`, `distribution.md` and `later.md`.

## 2026-10-27
- [x] Package registry decision: no central registry. Packages are Git repositories and versions are tags `vX.Y.Z`; `plm add`, `install`, `update`, `remove`, `tree` and `publish` work over Git, with `pleno.lock` recording each commit and a SHA-256 checksum, a shared cache and `--offline`. `crates/pleno-registry` and `PLENO_REGISTRY` are gone; the protocol is in `docs/specs/package-manager.md`
- [x] Web framework: `Router` and `Server` are stable. Middleware (`use`, with path prefixes, async, `next(err)`), `onError`, static files (`static`, with content types, `index.html`, `ETag`/`Last-Modified` and 304, no path traversal, sandbox read rule), cookies (`req.cookies`, `res.cookie`, `res.clearCookie`), `res.redirect`; header injection refused; errors no longer sent to the client; query strings and parameters decoded; 400/405/413/431 answers; chunked bodies. Example app in `examples/web_app`.
- [x] TCP sockets: `std:net` is stable. `Net.listen(host, port)` returns a `Listener` (`accept`, `address`, `port`, `close`) and `Net.connect(host, port)` a promise for a `Socket` (`read`, `readLine`, `write`, `setTimeout`, `close`, `localAddress`, `remoteAddress`). They run on the event loop with `await`; denied network access is a `SecurityError`; sockets close on `close()`, when collected and when the VM is dropped. Tests over loopback in `crates/pleno-vm/tests/stdlib_net_test.rs` and `tests/net_sockets_test.pln`.
- [x] Static checks: compile errors in the language server; calls on values of a known class, static methods, lambdas bound with `let` and functions imported from `.pln` files; names a `.pln` file does not export; type mismatches for instances and checked return types. Files that export are checked as modules. Static method calls bind their arguments like other calls, and compiling a large file is no longer quadratic.
- [x] `with` statement: `with (f = File.open(path), g = other()) { ... }` closes each resource in reverse order on every exit (end of block, `return`, `break`, `continue`, error); the block's error wins over close errors, which go to `e.suppressed`. `break` and `continue` now run `finally` blocks and pop the `try`'s handler.
- [x] Runtime type checks that were missing: `Void` returns, returns and closures that mention a type parameter (bound by explicit type arguments, any value when inferred), the type arguments of user generic classes in parameters, returns and fields, and `@inline` functions, which keep their checks when inlined. Inferred type parameters and instances made without type arguments no longer reject right values
- [x] Formatter: comments inside an index and in import and export lists stay in place, block comments in slices, parentheses and after a unary operator stay on the line
- [x] Language reference frozen for 1.x: a "frozen" section in `docs/specs/compatibility.md` (additions only, documented behavior kept, errata process). The freeze review checked the prose claims of every page and settled what was underspecified: closures share variables (they used to keep a stale copy), whole Floats print as `3.0`, strings inside containers print quoted, negative slice positions count from the end in every slice form, unknown string escapes are errors, `typeof` gives class names and `Function`, `Math.min`/`max` take every argument, `i64::MIN % -1` no longer crashes the compiler, the formatter keeps `1e21` a Float, and `std:db` obeys the sandbox. Newly documented: numbers (overflow, division, remainder, zero), string literals and escapes, how values print, scope and shadowing, reserved words, default parameters per call, closures and loop variables, map and set order, the recursion and nesting limits.
- [x] Pages for the experimental `std:gc` and `std:db` modules
- [x] Code Runner: `docs/guide/editor.md` shows the `code-runner.executorMap` setting for Pleno
- [x] Memory after a run (#60): dropping a VM frees every value it allocated, reference cycles included (#147), also cycles stored on the `Error` class (#149). Fuzzing runs with leak detection again (#155).
- [x] `==` compares arrays, tuples, maps and sets by contents, as `assert.equal` already did (#159)
- [x] Release 0.3.0: six targets, `.deb`, `.vsix` and Docker image, signed with cosign; the Homebrew tap updates itself; Scoop bucket at 0.3.0; package manifests in `packaging/` point at 0.3.0 (#146, #151)
- [x] Deep call chains and nested string interpolations are parse errors instead of stack overflows (#153); method values print without a memory address (#128)
- [x] `@pure` functions can build array, map and set literals (#142)
- [x] Language server: integration tests (`crates/pleno-lsp/tests/server.rs`) and workspace symbol search

## 2026-10-26
- [x] Differences between the language reference and the implementation fixed: `!=` uses `operator ==` and a sandbox limit cannot be caught (#68), string positions count characters (#76), sandbox path rules normalize paths and `File.exists` is restricted (#74), `@deprecated` warns (#101), generic constraints are checked for inferred type arguments and `T: Comparable` accepts `Int` (#105)
- [x] Std library gaps: `Regex` flags, `CSV.parse(text, { header: false })`, JWT `exp`/`nbf`/`alg` checks with constant-time comparison, `JSON.stringify` of class instances and sets, `XML.parse` errors, HTTP `HEAD`, number and Bool map literal keys (#103)
- [x] Error messages end with `; did you mean 'x'?` for close names (#113)
- [x] Destructuring patterns in `for` loops (#115); checked parameter annotations on lambdas (#119); inclusive `a[..=end]` slices (#121)
- [x] Runaway recursion and very long expressions are errors, not crashes (#78, #111); errors deep in the stack stay fast (#117)
- [x] A module cannot read the top-level names of the program that imports it

## 2026-10-25
- [x] Formatter keeps lines within 100 characters, breaking at the outermost call, literal, parameter list, import list, method chain or operator chain; a round-trip test formats every test, example and docs block (#70)
- [x] One error shape: every caught error is an `Error` instance with `name` and `message`, caught by `catch (e: Error)`; VM statistics moved to `vmStats()` in `std:reflect`
- [x] Destructuring in `let` (tuples, arrays with `...rest`, nesting, `_`) and tuple type annotations
- [x] Bytecode format version: `.plc` files start with `PLNC` and `BYTECODE_FORMAT_VERSION`; other versions and truncated files are rejected
- [x] Class and module rules enforced: imports bind only what they list, `internal`, inherited `protected`/`private`, abstract methods, `static const` and `const` fields
- [x] Runtime type annotations: `Int` to `Float`, return checks at every `return`, intersection types, `&mut` parameters cannot be reassigned
- [x] Int and Float equality as in Python (`5.0 == 5`, `9007199254740993 != 9007199254740992.0`)
- [x] Std library stability: every module marked stable or experimental in `docs/stdlib/index.md`
- [x] Language reference checked claim by claim; unimplemented designs moved to `notes/v2-draft.md`; `docs/specs/compatibility.md` written
- [x] Speed: faster than CPython 3.14 on every benchmark. Seconds, release build, best of 9 interleaved runs on one Windows machine, process start included (about 20 ms for Pleno, 35 ms for CPython):

  | Benchmark | Pleno before | Pleno after | CPython 3.14 |
  | --- | --- | --- | --- |
  | `bench/cases/fib` (fib(30)) | 0.151 | 0.090 | 0.099 |
  | `bench/cases/loop` (1M steps) | 0.040 | 0.029 | 0.078 |
  | `bench/cases/string` | 0.042 | 0.018 | 0.029 |
  | `bench/cases/collection` | 0.020 | 0.016 | 0.030 |
  | `bench/src/fib` | 0.118 | 0.074 | 0.080 |
  | `bench/src/loop` (10M steps) | 0.249 | 0.186 | 0.391 |
  | `bench/src/array` (1M elements) | 0.481 | 0.156 | 0.170 |
  | fib(27) plus a 3-million-step loop | 0.102 | 0.060 | 0.163 |

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
- [x] Undefined names are errors (`undefined variable 'foo'`) and `pleno check` reports them; calling a non-function names its type
- [x] Unhandled promise rejections fail the program; inputs to `Promise.race`/`Promise.all` count as handled
- [x] Failing `std:test` tests fail `pleno test`, `plm test` and `pleno run`
- [x] New projects: `plm test` finds `tests/`, the generated test uses `std:test`, `plm init` will not overwrite `pleno.toml`
- [x] Relative imports resolve from the importing file's folder, not the working directory
- [x] Colored CLI output with the logo gradient, only on terminals (`NO_COLOR`, `FORCE_COLOR`)
- [x] The compiler rejects malformed generic annotations instead of panicking (found by fuzzing)
- [x] `tasks/` reorganized into `v1.md`, `later.md`, `ideas.md` and `done.md`

## 2026-01-23
- [x] Setup project structure
- [x] Add basic CLI

## 2026-01-20
- [x] Initial parser
