# Road to Pleno 1.0

This page says what is left before Pleno 1.0, the first stable release. The detailed, item-by-item tracker is [`tasks/v1.md`](tasks/v1.md); the promise 1.0 makes is [`docs/specs/compatibility.md`](docs/specs/compatibility.md).

**What 1.0 means:** you can install Pleno with one command on Windows, macOS and Linux, write programs and libraries in it, and rely on it. Behavior is documented, does not change within 1.x, and the tools fail with a clear error instead of crashing or guessing.

Status as of 2026-10-26.

## Done

- **Language reference.** [`docs/specs`](docs/specs) describes only what is implemented, every code example in it runs in CI, and designs that are not implemented are collected in [`notes/v2-draft.md`](notes/v2-draft.md).
- **Compatibility promise** written ([`docs/specs/compatibility.md`](docs/specs/compatibility.md)): what 1.x keeps stable and what it does not (bytecode format, error wording, performance, experimental APIs).
- **Standard library stability**: every module is marked stable or experimental in [`docs/stdlib/index.md`](docs/stdlib/index.md). Experimental in 1.0: `std:net`, `std:security`, `std:gc`, `std:db`, the HTTP `Router`/`Server`, JWT and `vmStats`.
- **Errors**: every runtime, type and native error is an `Error` with a `name` and a `message`, caught by `catch (e: Error)`.
- **Language rules enforced**: runtime type annotations (with `Int` to `Float`), immutable bindings everywhere (also from closures), module privacy (imports bind only what they list; a module does not see its importer's names), `internal`/`protected`/`private`, abstract methods, `const` fields, destructuring in `let`, Int and Float equality.
- **Compile errors in unused code** are reported; the optimizer no longer hides them.
- **Speed**: faster than CPython 3.14 on every benchmark in `bench/` (fib, loops, strings, collections, arrays), also when an instruction limit is set.
- **Bytecode format** is versioned; old `.plc` files are rejected with a message to recompile.
- **Tools**: `pleno new`/`init`, `run`, `check`, `test`, `format`, `lint`, `compile`, `build`, `doc`, `repl`, language server, debugger, VS Code extension, and a script that installs everything from a checkout ([`scripts/dev-install.ps1`](scripts/dev-install.ps1), [`scripts/dev-install.sh`](scripts/dev-install.sh)).
- **Fuzzing**: four fuzz targets (lexer, parser, compiler, and running programs with both interpreter loops compared) run on every pull request that touches them.

## Left before 1.0

### Must have

1. **Fix the places where the implementation differs from the reference** (found while writing the reference; each has an issue or pull request):
   - [ ] String positions count bytes in `length`, `indexOf`, `substring`, `charAt` and slices, and some calls on non-ASCII text crash the VM. They must count characters everywhere. *(in progress)*
   - [ ] A sandbox limit could be caught with `try`; `!=` ignored a class's `operator ==`; `Sandbox.run` errors lost their name. *(fixed in a pull request, awaiting merge)*
   - [ ] Sandbox path allow-lists (`--allow-read`, `--allow-write`) compare text prefixes, so `data/../secret` passes a `data/` rule, and `File.exists` is not restricted. Paths must be normalized before checking.
   - [ ] `@deprecated` does nothing, but the compatibility promise says deprecated APIs warn. It needs to warn.
   - [ ] Generic constraints are not checked when type arguments are inferred, and `T: Comparable` rejects `Int`.
2. **Fuzzing time.** [ ] Long runs (6 hours per target) until a full day finds nothing. The first long run is in progress.
3. **Memory held after a run** ([#60](https://github.com/otabekoff/pleno/issues/60)). [ ] Values in reference cycles are not freed when a VM is dropped, so long-lived processes (language server, REPL, playground) grow.
4. **A release people can install.**
   - [ ] Release 0.3.0 with everything above, then 1.0.0-rc.1, then 1.0.0.
   - [ ] Signed Windows binaries (SignPath application ready in [`notes/SIGNPATH_APPLICATION.md`](notes/SIGNPATH_APPLICATION.md); waiting for approval).
   - [ ] Packages: winget (pull request waiting for moderators), Scoop, Homebrew (formula exists), a Linux package (apt), Chocolatey and AUR (need the maintainer's account keys).
   - [ ] The VS Code extension on the Marketplace and Open VSX.
5. **Freeze the reference.** [ ] After the fixes above, review `docs/specs` once more and declare it frozen for 1.x.

### Should have (1.0 or soon after)

- [ ] **Standard library gaps**: `Regex` flags argument, `CSV.parse` without a header, JWT `exp` checks and constant-time comparison, `JSON.stringify` of class instances and sets, `XML.parse` errors on malformed input, HTTP `HEAD`, number keys in map literals. *(in progress)*
- [ ] **Formatter line wrapping** (keeps lines within 100 characters). *(in a pull request, awaiting merge)*
- [ ] **Language**: destructuring in `for` loops (`for ((k, v) in pairs)`), checked annotations on lambda parameters, a `with` statement for resources that close themselves.
- [ ] **Errors with suggestions**: "did you mean ...?" for misspelled names and methods.
- [ ] **More static checks** in `pleno check` and the editor: calls on other objects, imported functions, inferred types.
- [ ] **Packages**: decide between a hosted registry and Git-based dependencies for 1.0, and document the protocol.
- [ ] **TCP sockets** (`std:net`): a real socket with `accept`, `read`, `write` and `close`, so it can leave experimental.
- [ ] **Web framework**: middleware, static files and cookies for `std:http`.
- [ ] **Code health**: split the VM's 7,000-line `lib.rs` into reviewable modules; replace `unsafe` blocks that do not pay for themselves with safe code.

## After 1.0

- Faster code from type annotations: integer-only instructions where the compiler knows a value is an `Int`.
- A JIT compiler for hot code (a 2.x project).
- Everything in [`notes/v2-draft.md`](notes/v2-draft.md).

## How to help

Pick an unchecked item, open an issue to say you are working on it, and see [`docs/guide/contributing.md`](docs/guide/contributing.md).
