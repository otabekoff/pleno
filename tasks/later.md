# After 1.0

Planned, but not needed for a stable 1.0. Status checked against the code on 2026-10-27.

## Performance

- [ ] **JIT compiler** (Cranelift or LLVM, x86-64 and ARM64). Status: not started; execution is the bytecode interpreter.
- [ ] **Generational GC**. Status: not started; `pleno-value/src/gc.rs` is a single-generation mark-and-sweep collector triggered by allocation count.
- [ ] **Object pooling** (`@pooled`). Status: parsed and recorded on the class, but allocation never uses a pool, and the annotation takes no arguments.
- [ ] **`Switch` opcode** for constant-time `match` dispatch. Status: not started; `match` compiles to a chain of comparisons and jumps.
- [ ] **`@hot` hint**. Status: stored in `FunctionProto.is_hot`, not used by any optimization. (`@inline`, `@no_optimize` and `@pure` work.)
- [ ] **Arena allocator for the AST**. Status: not started; AST nodes are `Box`ed.

## Security

- [ ] **OS-level sandboxing**. Status: `std:security` sandboxes run in a separate VM with I/O, network, memory and instruction limits, but not inside an OS sandbox (seccomp, job objects).
- [ ] **Per-module memory quotas**. Status: limits apply to a whole VM (`--max-memory`, `Sandbox({ maxMemory })`).

## Ecosystem and tools

- [ ] **std:db PostgreSQL driver**. Status: `std:db` supports SQLite (`sqlite:path`) only.
- [ ] **Macro expansion explorer**. Status: macros expand inside the compiler; no command prints the expanded program.
- [ ] **Language server refactorings** beyond rename. Status: not started.
- [ ] **WASM distribution**. Status: `crates/pleno-wasm` builds for `wasm32-unknown-unknown` and exports `compile`, `run` and `run_source`. It runs the docs playground and tutorial (`docs/playground.md`, built by `scripts/build-playground.sh` and tested in CI). Missing: an npm package.
- [ ] **Self-hosted compiler**. Status: not started. Needs a stable 1.0 language, fast enough execution, and file and string APIs good enough to write a compiler in Pleno.
