# Pleno v1 Roadmap

This roadmap stages the remaining implementation work for v1.

---

## Stage 8 - Functional Features (partial)

**Completed**: pipeline operator, basic pattern matching (identity/wildcard patterns), lambdas/closures (shorthand and anonymous fn expressions)

**Remaining**:
- Tuple patterns in match expressions ✅
- Full pattern matching runtime verification ✅
- Advanced pattern matching features

---

## Stage 9 - Async + Concurrency (not started)

**Completed**: async/await keywords parsed, Promise constructor exists

**Remaining**:
- Actual async execution
- Promise resolution
- Event loop
- Concurrency primitives

---

## Stage 10 - Standard Library (partial)

**Completed**: basic stdlib module stubs; JSON module fully implemented; I/O module (files/dirs) fully implemented; HTTP module fully implemented; Math and Regex libraries aligned with specs ✅

**Remaining**:
- Native function bindings for system operations (other than I/O and HTTP)

---

## Stage 11 - Tooling (partial)

**Completed**: LSP MVP (diagnostics, go-to-def); basic formatter/linter CLI commands; type checker CLI command; optional types (`Int?`) and union types (`Int | String`) runtime type checking

**Remaining**:
- Full formatter implementation in CLI ✅
- Full linter implementation (comprehensive style rules, best practices, unused variable detection)
- Docs tooling (generation, API docs)

---

## Stage 12 - Runtime + VM (partial)

**Completed**: register bytecode format; compiler for all language features; VM arithmetic/comparison/jumps; function call frames, class instances; exception handling; array methods (map, filter, reduce); 175+ passing tests; universal string concatenation; collection length aliases ✅

**Remaining**:
- GC integration (mark-sweep GC exists in pleno-gc crate but not integrated)
- Performance optimizations
- Memory management improvements

---

## Stage 13 - Core VM Crates (partial)

**Completed**: pleno-bytecode, pleno-compiler, pleno-vm (fully functional)

**Partial**:
- pleno-common (spans only)
- pleno-diagnostics (basic error reporting)
- pleno-value (primitives/collections/functions/classes)
- pleno-gc (exists but not integrated)

---

## Stage 14 - v1 Hardening (not started)

- Test suite expansion (currently 53+ tests, need more coverage)
- Error messages and diagnostics polish
- CLI parity with specs
- Performance benchmarking and optimization
- Documentation completion
