# Pleno V2 Roadmap

This roadmap outlines the journey toward Pleno V2, emphasizing stability, feature completeness, and enterprise readiness.

## Phase 1: Stability & Hardening (Month 1-2)

Goal: Make Pleno the most stable language in its class.

- **[ ] Fuzzing Infrastructure**: Implement continuous fuzzing for the Lexer and Parser.
- **[ ] Property-based Testing**: Integrate `proptest` for VM opcode verification.
- **[ ] Memory Safety Audit**: Conduct a full audit of all `unsafe` Rust code and GC logic.
- **[ ] Automated Benchmarking**: Prevent performance regressions with automated CI gating.

## Phase 2: Language Evolution (Month 3-5)

Goal: Implement modern syntax and type system improvements.

- **[ ] Optional Chaining & Nullish Coalescing**: Syntax support for safe null handling.
- **[ ] Generics V2**: Implementation of variance and advanced type constraints.
- **[ ] Enhanced Metaprogramming**: Faster and more flexible compile-time macros.
- **[ ] FFI Foundation**: Establish the base for calling external C/Rust libraries.

## Phase 3: Standard Library & Ecosystem (Month 6-8)

Goal: Build a library for real-world production use.

- **[ ] std:crypto**: Secure hashing and encryption.
- **[ ] std:db**: Unified database drivers (PostgreSQL, SQLite).
- **[ ] plm V2**: Package registry support and shared dependency resolution.
- **[ ] std:ui**: Prototype of a cross-platform UI/TUI library.

## Phase 4: Performance & Tooling (Month 9-12)

Goal: Achieve peak performance and professional DX.

- **[ ] JIT Compiler**: Baseline JIT for performance-critical code.
- **[ ] Professional LSP**: Full IDE support with go-to-definition and autocomplete.
- **[ ] Debugger (DAP)**: Integration with VS Code for step-through debugging.
- **[ ] WASM Backend**: Compile the Pleno VM to WebAssembly for edge compute.

---

_Survival of the fittest: Features may be adjusted based on community feedback and technical feasibility._
