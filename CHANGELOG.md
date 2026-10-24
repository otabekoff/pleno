# Changelog

Notable changes to Pleno. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Before 1.0, any release may change behavior.

## Unreleased

### Added

- `pleno check` and the language server report wrong argument counts and unknown named arguments for functions, constructors and `this` methods declared in the file, literals that cannot match a type annotation (`let x: Int = "hi";`), and, as a warning, unknown methods called on `this`.
- `pleno run file.plc` runs bytecode written by `pleno compile`.

### Changed

- `.plc` files start with a `PLNC` magic number and a bytecode format version, checked on load; files from another Pleno version, older headerless files and truncated files are rejected with an error asking to recompile.

## 0.2.0 - 2026-10-24

### Changed

- Reading an undefined name is an error (`undefined variable 'foo'`) instead of `null`, and `pleno check` reports undefined names.
- An unhandled promise rejection ends the program with an error instead of a warning.
- A failing `std:test` test fails `pleno test`, `plm test` and `pleno run`.
- Relative imports resolve from the importing file's folder, not the working directory.
- `plm publish` and `plm search` need `PLENO_REGISTRY`; there is no default registry.
- Uncaught exceptions print their message instead of the error map.
- Division and modulo by zero are errors for every Int and Float combination.
- A program whose top-level `await` can never finish ends with an error instead of exiting silently.
- Calling a method a map does not have names the method (`Map has no method 'contains'`) instead of `Null is not callable`.

### Added

- Backtick strings, generic type aliases, `extend <T> Array<T>`, enum methods after `;`, and `return` in match arms.
- `Result` and `Option` type annotations.
- Every standard library module can be imported as a namespace (`import { Path } from "std:path"`), plus `std:core` and `std:promise`.
- `DateTime` and `Duration`, `JSON.stringifyPretty`, `Regex.split`, `Benchmark.measure`, `Log.setLevel`, prefixed loggers, and collection helpers.
- `pleno check --json`; colored output on terminals (`NO_COLOR` and `FORCE_COLOR` are respected).
- VS Code extension 0.2.0, which can install the toolchain.
- A new documentation site with a tutorial and a playground that runs Pleno in the browser (WebAssembly). Every example in the docs is checked in CI.

### Fixed

- Infinity and NaN, 64-bit integers, enum type annotations, typed `catch` branches, contravariant interfaces, the pipeline operator with arguments, and iterating a channel.
- The formatter keeps comments, escapes and syntax it used to drop or break.
- Crashes on deeply nested code in the language server, and on malformed generic annotations in the compiler.
- A VM crash (index out of bounds) when a plain function returned a call to an async function that was waiting, and async functions called from `map`, `filter` and other built-ins returning `null` instead of their promise.
- `test()` with an `async fn` body reports its result when the body finishes, and a failure after an `await` fails the run.
- `Promise.all` put results in the wrong places, dropped some, and never finished when it mixed promises with plain values.
- Built-in functions called through a value (`xs |> map(f)`) could call the wrong function in the WebAssembly build.
- `std:test` output now goes wherever the program's output goes (the playground, the debugger).
- `Path.resolve` no longer returns `\\?\` paths or mixed separators on Windows; `Math.randomInt(n, n)` returns `n`; `unwrap()` on an `Err` shows the error.

## 0.1.0 - 2026-02-03

First release: the compiler, bytecode VM, standard library, `plm` package manager, language server, REPL and VS Code extension.
