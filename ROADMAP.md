# Road to Pleno 1.0

This page says what is left before Pleno 1.0, the first stable release. The detailed, item-by-item tracker is [`tasks/v1.md`](tasks/v1.md); the promise 1.0 makes is [`docs/specs/compatibility.md`](docs/specs/compatibility.md).

**What 1.0 means:** you can install Pleno with one command on Windows, macOS and Linux, write programs and libraries in it, and rely on it. Behavior is documented, does not change within 1.x, and the tools fail with a clear error instead of crashing or guessing.

Status as of 2026-10-27.

## Done

- **Language reference.** [`docs/specs`](docs/specs) describes only what is implemented, every code example in it runs in CI, and designs that are not implemented are collected in [`notes/v2-draft.md`](notes/v2-draft.md).
- **The reference and the implementation agree.** Every difference found while writing the reference is fixed: string positions count characters (#76); a sandbox limit cannot be caught and `!=` uses a class's `operator ==` (#68); sandbox path rules normalize paths (#74); `@deprecated` warns (#101); generic constraints are checked for inferred type arguments and `T: Comparable` accepts `Int` (#105); `@pure` accepts array, map and set literals (#142, awaiting merge).
- **Compatibility promise** written ([`docs/specs/compatibility.md`](docs/specs/compatibility.md)): what 1.x keeps stable and what it does not (bytecode format, error wording, performance, experimental APIs).
- **Standard library stability**: every module is marked stable or experimental in [`docs/stdlib/index.md`](docs/stdlib/index.md). Experimental in 1.0: `std:net`, `std:security`, `std:gc`, `std:db`, the HTTP `Router`/`Server`, and `vmStats`. The gaps found while documenting it are fixed (#103): `Regex` flags, `CSV.parse` without a header, JWT `exp` checks and constant-time comparison, `JSON.stringify` of class instances and sets, `XML.parse` errors, HTTP `HEAD`, number keys in map literals.
- **Errors**: every runtime, type and native error is an `Error` with a `name` and a `message`, caught by `catch (e: Error)`. Misspelled names get "did you mean ...?" (#113). Runaway recursion and very long expressions are errors instead of crashes (#78, #111), and errors deep in the stack stay fast (#117).
- **Language rules enforced**: runtime type annotations (with `Int` to `Float`, also on lambda parameters, #119), immutable bindings everywhere, module privacy, `internal`/`protected`/`private`, abstract methods, `const` fields, destructuring in `let` and `for` (#115), Int and Float equality, inclusive `a[..=end]` slices (#121).
- **Compile errors in unused code** are reported; the optimizer no longer hides them.
- **Speed**: faster than CPython 3.14 on every benchmark in `bench/` (fib, loops, strings, collections, arrays), also when an instruction limit is set.
- **Bytecode format** is versioned; old `.plc` files are rejected with a message to recompile.
- **Tools**: `pleno new`/`init`, `run`, `check`, `test`, `format` (keeps lines within 100 characters), `lint`, `compile`, `build`, `doc`, `repl`, language server, debugger, VS Code extension, and a script that installs everything from a checkout ([`scripts/dev-install.ps1`](scripts/dev-install.ps1), [`scripts/dev-install.sh`](scripts/dev-install.sh)).
- **Fuzzing**: four fuzz targets (lexer, parser, compiler, and running programs with both interpreter loops compared) run on every pull request that touches them. The bugs they found are fixed.
- **Dependencies** are current; Dependabot groups updates and skips versions that cannot build (TypeScript 6.1+ until typescript-eslint supports it, the bincode 3 placeholder).

## Left before 1.0

### Must have

1. **Fuzzing time.** [ ] Long runs until a full day finds nothing. A 5.5-hour run on `main` started on 2026-10-27.
2. **Memory held after a run** ([#60](https://github.com/otabekoff/pleno/issues/60)). [ ] Values in reference cycles are not freed when a VM is dropped, so long-lived processes (language server, REPL, playground) grow. *(in progress)*
3. **Freeze the reference.** [ ] A last review of `docs/specs` by the maintainer, then declare it frozen for 1.x.
4. **A release people can install.**
   - [x] Release 0.3.0 (2026-10-27).
   - [ ] Release 1.0.0-rc.1, then 1.0.0.
   - [ ] Signed Windows binaries (SignPath application ready in [`notes/SIGNPATH_APPLICATION.md`](notes/SIGNPATH_APPLICATION.md); waiting for approval).
   - [ ] Packages: winget (pull request waiting for moderators), Scoop, Homebrew (formula exists), a Linux package (apt), Chocolatey and AUR (need the maintainer's account keys).
   - [ ] The VS Code extension on the Marketplace and Open VSX.

### Should have (1.0 or soon after)

- [ ] **`with` statement** for resources that close themselves.
- [ ] **More static checks** in `pleno check` and the editor: calls on other objects, imported functions, inferred types.
- [ ] **Packages**: decide between a hosted registry and Git-based dependencies for 1.0, and document the protocol.
- [ ] **TCP sockets** (`std:net`): a real socket with `accept`, `read`, `write` and `close`, so it can leave experimental.
- [ ] **Web framework**: middleware, static files and cookies for `std:http`.
- [ ] **Language server**: integration tests and workspace symbol search.
- [ ] **Code health**: split the VM's 8,000-line `lib.rs` into reviewable modules; replace `unsafe` blocks that do not pay for themselves with safe code.

## After 1.0

- Faster code from type annotations: integer-only instructions where the compiler knows a value is an `Int`.
- A JIT compiler for hot code (a 2.x project).
- Everything in [`notes/v2-draft.md`](notes/v2-draft.md).

## How to help

Pick an unchecked item, open an issue to say you are working on it, and see [`docs/guide/contributing.md`](docs/guide/contributing.md).
