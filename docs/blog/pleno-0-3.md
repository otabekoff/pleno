---
title: "Pleno 0.3.0: faster than CPython, and saying what it means"
date: 2026-10-27
author: Otabek
description: Pleno now beats CPython 3.14 on every benchmark, the language reference and the implementation agree, and it installs with Homebrew and Scoop.
---

Pleno 0.3.0 is out. It is the release before 1.0: the language does what its
[reference](/specs/) says, the reference describes only what exists, and the tools give a clear
error instead of guessing.

```sh
brew install otabekoff/tap/pleno        # macOS, Linux
```

```powershell
scoop bucket add pleno https://github.com/otabekoff/scoop-bucket
scoop install pleno                      # Windows
```

Or download from [the release page](https://github.com/otabekoff/pleno/releases/tag/v0.3.0),
where every file is signed with cosign. [Installation](/guide/installation) has the other ways.

## Faster than CPython

Pleno 0.2 was slower than CPython. 0.3.0 is faster on every benchmark in the repository: `fib(30)`
runs in 0.09 s against CPython 3.14's 0.10 s, and a million-step loop in 0.03 s against 0.08 s. A
10-million-step loop went from 0.9 s to 0.26 s, and `map`, `filter` and `reduce` over a million
elements are about three times faster. The interpreter passes arguments in registers, calls methods
without allocating, and runs simple instructions in a small, tight loop.

## The language

- **Destructuring:** `let (a, b) = pair;`, `let [first, ...rest] = items;` and
  `for ((key, value) in map)`.
- **Checked types where you write them:** return annotations and lambda parameters are checked when
  the code runs (`TypeError: f returned String, expected Int`). Generic constraints are checked
  too, and `T: Comparable` covers numbers, strings and classes with `operator <`.
- **`==` compares contents:** `[1, [2]] == [1, [2]]` and `{"a": 1} == {"a": 1}` are `true`, as
  in Python, and `5 == 5.0` is `true`. Class instances still compare by identity, unless their
  class defines `operator ==`.
- **One kind of error:** everything a `catch` receives is an `Error` with a `name` and a
  `message`. Misspelled names end with `did you mean 'length'?`.
- **Strings count characters, not bytes,** everywhere.
- **Rules that were documented are now enforced:** immutable bindings stay immutable inside
  closures, an import binds only the names it lists, and `private`, `protected` and `internal`
  mean what they say.
- **`@deprecated` warns.**

## Safer

- Runaway recursion, very long expressions and deeply nested code are errors, not crashes.
- A sandbox limit cannot be caught to keep running, and sandbox path rules can no longer be
  escaped with `..`.
- Dropping a VM frees all of its memory, so the language server and the playground no longer grow.
- Four fuzz targets run on every change, with AddressSanitizer and leak detection. Every crash they
  found is fixed.

## Tools

- `pleno new my-app` creates a project in a new folder.
- `pleno format` keeps lines within 100 characters.
- `pleno check` and the editor catch wrong argument counts, unknown named arguments and literals of
  the wrong type before you run anything.
- Every command rejects options it does not know instead of ignoring them.

The full list is in the [changelog](https://github.com/otabekoff/pleno/blob/main/CHANGELOG.md).

## Next: 1.0

What is left is in [the roadmap](https://github.com/otabekoff/pleno/blob/main/ROADMAP.md):

- freezing the language reference for 1.x;
- a full day of fuzzing that finds nothing;
- the VS Code extension in the Marketplace and Open VSX;
- WinGet;
- signed Windows binaries.
