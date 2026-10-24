# Pleno V2: open work and notes

Status checked against the code on 2026-10-24. Completed items have been removed; see `tasks/done.md` for what was finished.

## Open work

### Language

- [ ] **Tuple types and destructuring**. Status: tuples, `.0` access and named tuples work. Missing: tuple type annotations (`let p: (String, Int)`) and destructuring (`let (a, b) = p;`, `let [x, y] = arr;`), which `notes/specs.md` 5.4 describes.
- [ ] **`with` statement** for resources that close themselves (`with (f = File.open(...)) { ... }`). Status: not started; the docs mark it as planned.
- [ ] **One error shape**. Status: runtime errors are thrown as maps with `message`, but compiler-generated type errors are thrown as strings, so `catch (e) { e.message }` works for one and not the other.

### Performance

- [ ] **JIT compilation** (spec 19.1). Status: not started. Execution is the bytecode interpreter only. A JIT also needs a design (Cranelift or LLVM; see the roadmap).
- [ ] **Generational GC** (spec 19.2). Status: not started. `pleno-value/src/gc.rs` is a single-generation mark-and-sweep collector triggered by allocation count.
- [ ] **Object pooling** (spec 12.3, `@pooled`). Status: parsed only. The compiler records `@pooled` on the class (`SetPooled`), but allocation never uses a pool.
- [ ] **`Switch` opcode** for O(1) `match` dispatch. Status: not started. `match` compiles to a chain of comparisons and jumps.
- [ ] **`@hot`** hint. Status: stored in `FunctionProto.is_hot`, not used by any optimization. (`@inline`, `@no_optimize` and `@pure` are implemented.)

### Security

- [ ] **OS-level sandboxing** (spec 20.2). Status: not started. `std:security` sandboxes run in a separate VM with I/O, network, memory and instruction limits, but not in an OS-level sandbox (seccomp, job objects).
- [ ] **Per-module memory quotas**. Status: not started. Memory limits apply to a whole VM (`--max-memory`, `Sandbox({ maxMemory })`).

### Tooling

- [ ] **Macro expansion explorer**. Status: not started. Macros are expanded inside the compiler; there is no command that prints the expanded program.
- [ ] **Central package registry**. Status: partly built. `crates/pleno-registry` is a registry server (Axum + SQLite), and `plm` can query and publish to a registry URL, but no public registry is deployed and the protocol is undocumented.
- [ ] **LSP completeness**. Status: definition, references, rename, hover, completion, signature help, inlay hints, semantic tokens, folding, formatting, code lens, call hierarchy and code actions are implemented. Missing: refactorings beyond rename, workspace-wide symbol search, and integration tests for the server.
- [ ] **Formatter**. Status: every test and example still passes after formatting, a second pass changes nothing, and comments, doc comments and blank lines between statements and members are kept. Missing: comments inside a multi-line expression (an array or call split over lines) move below the statement, and long lines are not wrapped (`max_line_length` is unused).
- [ ] **Error messages with suggestions**. Status: errors report the right line, operand types, and the actual type in type errors (`expected Color, found Size`). Missing: "did you mean" suggestions, and the name of the value in "value is not callable".
- [ ] **`pleno check` coverage**. Status: reports lex, parse and compile errors with positions (`--json` for editors). It does not report undefined names: `println(1);` passes `check` and fails at run time.

### Standard library

- [ ] **Express-like web framework**. Status: `std:http` has a router with `:param` routes, query strings, headers, bodies, `res.status/header/send/json` and async handlers. Missing: middleware (`app.use`), static files, cookies.
- [ ] **TCP sockets (`std:net`)**. Status: `listen` binds and `connect` connects, but both close the socket at once and return a plain map. Missing: a socket resource with `accept`, `read`, `write` and `close`.
- [ ] **Code Runner support**. Status: not done. The VS Code extension has its own "Run" command; Code Runner's executor map is a user setting the extension cannot safely change.

## Ideas and notes

And make a framework like express in nodejs.

LLVM or cranelift support.

Unicode, emojis, gradients, pleno ascii text.

LSP and IDE features. REPL, also colors?.

More comment types?

New folder called prompts.

V2, we'll be the most ideal one.

Add more licenses from rust like githuib stuff.

I heard somewhere, making everything Boxed or boexed expressions in programming language is not good way and it makes slow. Is it true? Why? How can we avoid it? Is it possible to avoid it? How can we make it better? Do we have such cases or not? If we have such places, do we need to make them better and avoid or are there the places where they should be like so?

To earn at a stage where we can write compiler of Pleno with itself, what should we do?

How we can optimize the code?

How we can make Pleno very fast even if it is dynamic?

Move to new version of taks management in tasks folder. New workflow.

We've to think about bad practices, inconsistencies. We've to review what we have already and reconsider everything. Need to find inconsistencies, duplicated, bad dx and practices and fix and solve and upgrade, update them.

Pleno V2 will be made withh Rust too, but not connected to Rust. Meaning we'll use Rust to make Pleno v2, but it won't be connected to Rust. Meaning it works even if there is no Rust installed on the system. Pleno needs to reach a level where we can write its compiler with itself. Will have self-hosted compiler and at the end we'll get rid of Rust. Pleno will become independent programming language.

## Questions

- To reach self-hosted compiler programming level what we need to do?
- To make this programming language very fast what we need to do?
