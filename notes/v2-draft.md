# Pleno: draft for after 1.0

This file collects features that are designed or wanted but not implemented, so the 1.0 reference in `docs/specs/` can describe only what exists. Nothing here is part of the 1.0 language or covered by the compatibility promise (`docs/specs/compatibility.md`). Each item has a one-line status. Some items can land in a 1.x minor release if they only add behavior; anything that changes the meaning of valid 1.0 code waits for 2.0.

Code blocks here are sketches and do not run.

## Language

### `with` statement

Status: not started.

A block that closes a resource when it ends, also on an error:

```text
with (file = File.open("data.txt", "r")) {
    print(file.read());
}   // file.close() runs here
```

Until then, close handles in `finally`.

### Iterating strings

Status: not started. `for (c in "abc")` is a compile error (`for loop expects array, set, or range`) and a string variable throws `type 'String' is not iterable`. Planned: iterate Unicode characters.

### Byte-level string access

Status: not started. There are no byte APIs (`bytes()`, `byteLength`). Every string position and length counts Unicode characters.

### Maps with non-string keys

Status: map keys are strings. Number and Bool literal keys are converted to strings (`{1: "a"}` is `{"1": "a"}`), and so are number and Bool indexes (`m[1]`). Not started: maps keyed by other values (numbers that stay numbers, tuples, objects), which would change `keys()` and iteration.

### Identity comparison

Status: `==` compares arrays, tuples, maps and sets by contents (decided before 1.0, so `==` agrees with `assert.equal`), and class instances by identity unless their class defines `operator ==`. There is no way to ask whether two arrays or maps are the same object. Planned: an identity check, such as a `same(a, b)` function in `std:reflect` or an `===` operator, so code that needs identity does not depend on `==`.

### Purity checked by the compiler

Status: `@pure` has an incomplete runtime check (experimental in 1.0). Planned: a compile-time check that rejects side effects (I/O natives, assignments outside the function, calls to non-pure functions and mutating methods on values the function does not own) and accepts building new arrays and maps.

### Checked built-in annotations

Status: `@override` and `@sealed` are plain annotations with no effect. (`@deprecated` warns since 2026-10-26.)

- `@override`: a compile error when no parent class has the method.
- `@sealed`: a class that cannot be extended.
- `@deprecated` on classes, fields and imported functions in `pleno check` (calls to deprecated top-level functions of the same file and every call at run time already warn).

### More runtime type checks

Status: partial.

- Lambda parameter annotations that name a type parameter of an enclosing function (`fn f<T>() { let g = (x: T) => x; }`) are not checked.
- Return annotations are not checked for `Void`, for annotations that mention a type parameter, and for `@inline` functions.
- With inferred type arguments, constraints are checked for parameters annotated `T`, not for the elements of a `T[]` parameter.

### Static type checking

Status: `pleno check` finds undefined names, wrong argument counts and contradicting literals. Planned: inferred variable and return types, calls on other receivers and imported functions, annotations naming classes, aliases and generic types.

### Memory pooling (`@pooled`)

Status: `@pooled` is an annotation with no effect.

```text
@pooled(size: 100)
class Particle { x: Float; y: Float; }
```

Instances would be taken from and returned to a pool. Also proposed: an `Arena` handle for batch allocation.

### Macros

Status: `macro name(...) { ... }` and `name!(...)` work for simple code substitution (see `docs/specs/advanced-features.md`). Not designed: hygiene, macro imports and exports, and error locations inside expansions.

### Foreign function interface

Status: not started.

```text
import { FFI } from "std:ffi";
let libc = FFI.load("libc.so.6");
let puts = libc.bind("puts", [Type.String], Type.Int);
```

## Standard library

### TCP sockets (`std:net`)

Status: experimental in 1.0. `Net.listen` binds and `Net.connect` connects, but both close the socket at once and return a plain map.

Planned API:

| Call | Returns |
| --- | --- |
| `Net.listen(addr)` | `Listener` with `address`, `accept()`, `close()` |
| `Net.connect(addr)` | `Connection` with `remoteAddress`, `read()`, `write(data)`, `close()` |

### HTTP server

Status: `Router` and `Server` are experimental in 1.0. Missing: middleware (`app.use`), static files and cookies.

### Other module work

- `std:gc`, `std:db`, `std:security`: experimental, undocumented or thinly documented; decide their 1.x shape.
- Proposed new modules: `std:grpc` (gRPC and Protobuf), `std:ui` (terminal and simple desktop UI), PostgreSQL in `std:db`.

### Sandboxing

Status: `std:security` is experimental. Planned: a time limit. (Path rules normalize paths, `Path.exists` and the other path probes are checked, and `Sandbox.run` errors keep their name, since 2026-10-26.)

## Tooling

- `pleno format --check` for CI, and `pleno lint --fix`.
- `pleno test --coverage`.
- `pleno check` on a folder.
- `pleno doc --serve`.
- Workspaces in `pleno.toml` (`[workspace] members = [...]`).
- Packages are Git repositories with tags (decided for 1.0; see `docs/specs/package-manager.md`). Later, optionally: a package index as a plain Git repository.
- `plm test --fuzz` and property-based testing.
- Language server: workspace symbol search, compile errors and undefined names while typing.

## Runtime and performance

- A baseline JIT compiler for x86_64 and ARM64.
- Generational and concurrent garbage collection.
- `@hot`: let the VM prioritize a function (status: recorded, no effect).
- Faster calls (smaller frames), and closures created in a loop that share their code.

## Bugs that block the freeze

None known. These are differences between the reference and the implementation (the reference describes the intended behavior); list any that are found here.
