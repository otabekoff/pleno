# Road to Pleno 1.0

This page says what is left before Pleno 1.0, the first stable release. Only open work is listed.
What is done is in [`CHANGELOG.md`](CHANGELOG.md) and [`tasks/done.md`](tasks/done.md).

The details of each item, what exists and what is missing, are in
[`tasks/v1.md`](tasks/v1.md). Releases, signing, package managers and the extension stores have
their own tracker, [`tasks/distribution.md`](tasks/distribution.md). The promise 1.0 makes is
[`docs/specs/compatibility.md`](docs/specs/compatibility.md).

**What 1.0 means:** you can install Pleno with one command on Windows, macOS and Linux, write
programs and libraries in it, and rely on it. Behavior is documented, does not change within 1.x,
and the tools fail with a clear error instead of crashing or guessing.

Status as of 2026-10-27. The latest release is [0.3.0](https://github.com/otabekoff/pleno/releases/tag/v0.3.0).

## Left before 1.0

### Must have

1. **Freeze the reference.** [ ] The maintainer's last review of `docs/specs`, including the
   decisions made for 1.0 (`==` compares containers by contents), then declare it frozen for 1.x.
2. **Fuzzing time.** [ ] Long runs that find nothing for a full day in total. Every crash found
   so far is fixed, and leak detection is on. A 5.5-hour run on `main` started on 2026-10-27.
3. **A release people can install.** [ ] 1.0.0-rc.1, then 1.0.0, with signed Windows binaries,
   WinGet, the VS Code Marketplace and Open VSX. Every channel's status and open step:
   [`tasks/distribution.md`](tasks/distribution.md).

### Should have (1.0 or soon after)

- [ ] **More static checks** in `pleno check` and the editor: compile errors in the language
  server, calls on other objects and imported functions, inferred types.
- [ ] **Runtime type checks** still missing: `Void` returns, annotations with type parameters,
  type arguments of generic classes, `@inline` functions.
- [ ] **Speed**: Pleno is already faster than CPython on every benchmark. Left: cheaper calls, the
  fast loop while debugging, closures that share their code.
- [ ] **Web framework**: middleware, static files and cookies for `std:http`, so `Router` and
  `Server` can become stable.
- [ ] **`with` statement** for resources that close themselves.
- [ ] **Packages**: decide between a hosted registry and Git-based dependencies for 1.0, and
  document the protocol.
- [ ] **Error suggestions** in the places not covered yet (enclosing functions' locals, other
  modules' exports, type names).
- [ ] **Formatter**: comments inside an index or an import list stay in place.
- [ ] **Code health**: split the VM's 8,000-line `lib.rs` into reviewable modules; replace `unsafe`
  blocks that do not pay for themselves with safe code.
- [ ] **Docs** for the experimental `std:gc` and `std:db` modules, and for using Code Runner with
  Pleno.
- [ ] **More package channels**: Chocolatey, AUR and an apt repository
  ([`tasks/distribution.md`](tasks/distribution.md)).

## After 1.0

- Faster code from type annotations: integer-only instructions where the compiler knows a value
  is an `Int`.
- A JIT compiler for hot code (a 2.x project).
- An identity check (`same(a, b)` or `===`), now that `==` compares containers by contents.
- Snap, Flatpak, RPM, asdf and a JetBrains plugin.
- Everything in [`notes/v2-draft.md`](notes/v2-draft.md).

## How to help

Pick an unchecked item, open an issue to say you are working on it, and see
[`docs/guide/contributing.md`](docs/guide/contributing.md).
