# Changelog

Notable changes to Pleno. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Before 1.0, any release may change behavior.

## Unreleased

### Changed

- Reading an undefined name is an error (`undefined variable 'foo'`) instead of `null`, and `pleno check` reports undefined names.
- An unhandled promise rejection ends the program with an error instead of a warning.
- A failing `std:test` test fails `pleno test`, `plm test` and `pleno run`.
- Relative imports resolve from the importing file's folder, not the working directory.
- `plm publish` and `plm search` need `PLENO_REGISTRY`; there is no default registry.
- Uncaught exceptions print their message instead of the error map.
- Division and modulo by zero are errors for every Int and Float combination.

### Added

- Backtick strings, generic type aliases, `extend <T> Array<T>`, enum methods after `;`, and `return` in match arms.
- `Result` and `Option` type annotations.
- Every standard library module can be imported as a namespace (`import { Path } from "std:path"`), plus `std:core` and `std:promise`.
- `DateTime` and `Duration`, `JSON.stringifyPretty`, `Regex.split`, `Benchmark.measure`, `Log.setLevel`, prefixed loggers, and collection helpers.
- `pleno check --json`; colored output on terminals (`NO_COLOR` and `FORCE_COLOR` are respected).
- VS Code extension 0.2.0, which can install the toolchain.

### Fixed

- Infinity and NaN, 64-bit integers, enum type annotations, typed `catch` branches, contravariant interfaces, the pipeline operator with arguments, and iterating a channel.
- The formatter keeps comments, escapes and syntax it used to drop or break.
- Crashes on deeply nested code in the language server, and on malformed generic annotations in the compiler.

## 0.1.0 - 2026-02-03

First release: the compiler, bytecode VM, standard library, `plm` package manager, language server, REPL and VS Code extension.
