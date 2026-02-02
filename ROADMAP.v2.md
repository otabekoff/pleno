# Pleno V2 Roadmap

This roadmap outlines the journey toward Pleno V2, emphasizing stability, feature evolution, and high-performance execution.

---

## Phase 1: Stability & Hardening (Month 1-2)

**Goal:** Bulletproof the core engine and automate quality control.

- **[ ] Fuzzing Infrastructure**: Continuous fuzzing for Lexer and Parser nodes.
- **[ ] Property-based Testing**: VM opcode verification using `proptest`.
- **[ ] Memory Safety Audit**: Comprehensive review of `unsafe` blocks and GC logic.
- **[ ] Arena Allocator**: Implement arena allocation for the AST to eliminate `Box<T>` overhead.
- **[ ] Automated Benchmarking**: prevent performance regressions with CI gating.

## Phase 2: Language Evolution (Month 3-5)

**Goal:** Implement modern syntax and reified type system features.

- **[ ] Operator Overloading**: Support `operator +`, `==`, `[]`, etc., in class definitions.
- **[ ] Null Safety**: Implementation of Optional Chaining (`?.`) and Nullish Coalescing (`??`).
- **[ ] If/Match Expressions**: Turn control flow statements into full expressions.
- **[ ] Custom Decorators**: Allow users to define their own `@decorator` functions.
- **[ ] Generics V2**: Support for variance (`out T`, `in T`) and reified runtime types.
- **[ ] Ownership Hints**: Optional `move` and `ref` keywords for explicit memory control.

## Phase 3: Standard Library & Ecosystem (Month 6-8)

**Goal:** Expand the toolkit for production-level application development.

- **[ ] std:crypto**: Native support for SHA, AES, and JWT primitives.
- **[ ] std:db**: Unified interface for SQLite and PostgreSQL.
- **[ ] Enhanced std:http**: Async event loop integration and full Router callback support.
- **[ ] Serialization**: Native, high-performance CSV, TOML, and YAML modules.
- **[ ] plm registry**: Design and implement the central Pleno package registry.

## Phase 4: Performance & Runtime (Month 9-12)

**Goal:** Match interpreted language performance leaders.

- **[ ] JIT Compiler**: Baseline JIT targeting x86_64/ARM64 (via Cranelift).
- **[ ] Generational GC**: Shift from simple mark-and-sweep to a generational/concurrent collector.
- **[ ] Optimization Passes**: Implement Inlining, Constant Folding, and Dead Code Elimination.
- **[ ] NaN Boxing**: Optimize value representation for 64-bit platforms.

## Phase 5: Developer Experience (Ongoing)

**Goal:** Provide professional-grade tooling and debugging.

- **[ ] Professional LSP**: Full refactoring, go-to-definition, and inlay hits.
- **[ ] Debugger (DAP)**: VS Code integration for step-through debugging.
- **[ ] WASM Backend**: High-performance execution in browsers and at the edge.

---

_Note: Features and timelines are living targets and will evolve based on community feedback._
