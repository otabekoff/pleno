# Compatibility promise

This page says what you can rely on when you write Pleno code against version 1.0, and what can still change in a 1.x release.

## Versions

Pleno uses [semantic versioning](https://semver.org): a version is `MAJOR.MINOR.PATCH`.

- A **patch** release (1.0.1) fixes bugs.
- A **minor** release (1.1.0) adds features and may deprecate old ones.
- A **major** release (2.0.0) may remove deprecated features and change behavior.

`pleno --version` prints the version you have. Before 1.0, any release could change behavior; see the [changelog](https://github.com/otabekoff/pleno/blob/main/CHANGELOG.md).

## What 1.x guarantees

- **Programs keep their meaning.** A program that is valid under the 1.0 language reference (the pages in this section) runs and behaves the same way under every 1.x release: same syntax, same results, and errors are thrown in the same situations.
- **Stable standard library APIs stay.** A function, class, method or constant the [standard library list](/stdlib/) marks stable keeps its name, its parameters and its documented behavior in every 1.x release. New functions, new optional parameters and new fields on returned maps can be added.
- **Deprecation before removal.** A stable feature or API is not removed in 1.x. It can be deprecated in a minor release; it then warns (from `pleno check` or `pleno lint`, or when it runs) for at least one minor release, keeps working for the rest of 1.x and can be removed in 2.0. The changelog lists every deprecation.

## What it does not cover

- **Bytecode.** The `.plc` format and its format version can change in any release, minor or patch. Keep your `.pln` sources and recompile with `pleno compile` after upgrading; a `.plc` file from another version is rejected with an error, never misread.
- **Error message wording.** The `name` of a built-in error (`TypeError`, `SecurityError`, ...) and the situations that throw are covered; the text of `message`, of compile errors and of warnings is not. Do not parse it.
- **Printed output meant for people:** log lines, benchmark reports, test runner output, `--help` text. Machine-readable output (`--json`) only gains fields.
- **Performance.** Speed and memory use can change in either direction.
- **Experimental modules and APIs.** They can change or disappear in any release (see below).
- **Internal Rust crates.** The `pleno-*` crates in the repository have no stable Rust API; only the `pleno` and `plm` commands and the language are covered.
- **Unspecified behavior.** Where the reference says the result is unspecified (for example the order of `Directory.list`), a later release can give a different result.
- **Bug fixes.** When the implementation does not do what the reference says, a 1.x release can fix the implementation, even if a program depended on the wrong behavior. Fixes that change behavior are listed in the changelog.
- **New checks in `pleno check` and `pleno lint`.** Later versions can report more problems in the same code; that does not change how the code runs.

## Experimental APIs

An experimental module or function is outside the promise. Its documentation page starts with

> **Stability:** experimental. Not covered by the 1.x compatibility promise.

and an experimental function inside a stable module starts its description with **Experimental.** The [standard library list](/stdlib/) has a stability column for every module. An experimental API becomes stable in a minor release once it is finished and tested; the changelog says when.

Pages in the "Implementation notes" group of this reference (performance features, bytecode VM, distribution plan, release plan) describe the current implementation or plans. They are not part of the promise.

## Not yet in 1.0

Features that are designed but not implemented are collected in the [version 2 draft](https://github.com/otabekoff/pleno/blob/main/notes/v2-draft.md). They are not part of the 1.0 reference, and a program that uses them is not valid 1.0 code.
