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

### Patterns in `for` loops

Status: not started. `let` destructuring works; `for` does not take patterns.

```text
for ((name, score) in pairs) { ... }
for ([first, ...rest] in rows) { ... }
```

### Iterating strings

Status: not started. `for (c in "abc")` is a compile error (`for loop expects array, set, or range`) and a string variable throws `type 'String' is not iterable`. Planned: iterate Unicode characters.

### Byte-level string access

Status: not started. There are no byte APIs (`bytes()`, `byteLength`). Positions are meant to count Unicode characters; see "Bugs that block the freeze" below.

### Maps with non-string keys

Status: map keys are strings. Number and Bool literal keys are converted to strings (`{1: "a"}` is `{"1": "a"}`), and so are number and Bool indexes (`m[1]`). Not started: maps keyed by other values (numbers that stay numbers, tuples, objects), which would change `keys()` and iteration.

### Purity checked by the compiler

Status: `@pure` has an incomplete runtime check (experimental in 1.0). Planned: a compile-time check that rejects side effects (I/O natives, assignments outside the function, calls to non-pure functions and mutating methods on values the function does not own) and accepts building new arrays and maps.

### Checked built-in annotations

Status: `@override`, `@sealed` and `@deprecated` are plain annotations with no effect.

- `@override`: a compile error when no parent class has the method.
- `@sealed`: a class that cannot be extended.
- `@deprecated("message")`: a warning from `pleno check`, `pleno lint` and the language server where the name is used. The 1.x compatibility promise needs this mechanism for deprecations.

### More runtime type checks

Status: partial.

- Lambda parameter annotations are not checked.
- Return annotations are not checked for `Void`, for annotations that mention a type parameter, and for `@inline` functions.
- Generic constraints (`T: Shape`) are checked only with explicit type arguments; inferred type arguments are not checked.

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

Status: `std:security` is experimental. Planned: path rules that normalize paths (today `--allow-read data/` is a string prefix test that `data/../x` passes), a time limit, and restricting `File.exists`.

## Tooling

- `pleno format --check` for CI, and `pleno lint --fix`.
- `pleno test --coverage`.
- `pleno check` on a folder.
- `pleno doc --serve`.
- Workspaces in `pleno.toml` (`[workspace] members = [...]`).
- A public package registry or a documented Git-only answer (`crates/pleno-registry` exists, none is deployed).
- `plm test --fuzz` and property-based testing.
- Language server: workspace symbol search, compile errors and undefined names while typing.

## Runtime and performance

- A baseline JIT compiler for x86_64 and ARM64.
- Generational and concurrent garbage collection.
- `@hot`: let the VM prioritize a function (status: recorded, no effect).
- Faster calls (smaller frames), and closures created in a loop that share their code.

## Bugs that block the freeze

These are not features but differences between the reference and the implementation; the reference describes the intended behavior.

- String positions: `length`, `indexOf`, `substring`, `charAt` and `s[a..b]` count UTF-8 bytes while `s[i]` counts characters; `substring` and `charAt` inside a multi-byte character panic the VM. `"abc".split("")` returns `["", "a", "b", "c", ""]`.
- `@pure` rejects non-empty array and map literals.
- Generic constraints report `Type 'Unknown'` for a class's explicit type argument (`Box<Int>(5)`), and `T: Comparable` rejects `Int` although `Int` is marked as implementing `Comparable` internally.
