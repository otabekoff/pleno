# Pleno v1 Roadmap

This roadmap tracks remaining implementation work for v1. Completed stages have been archived.

---

## Completed Stages ✅

- **Stage 8 - Functional Features**: pipeline operator, pattern matching, lambdas/closures
- **Stage 9 - Async + Concurrency**: async/await, promises, event loop, Mutex, Channels, spawn()
- **Stage 10 - Standard Library**: JSON, I/O, HTTP, Math, Regex, OS, DateTime, std:path, std:process, Map/Set functional API
- **Stage 11 - Tooling**: LSP, formatter, linter, type checker, docs tooling

---

## Stage 12 - Runtime + VM (mostly complete)

**Completed**: register bytecode format; compiler for all language features; VM arithmetic/comparison/jumps; function call frames, class instances; exception handling; array methods (map, filter, reduce); 193 passing tests; universal string concatenation; collection length aliases; memory management ✅

**Remaining**:
- Performance optimizations (optional for v1)
- NaN boxing (optional for v1)

---

## Stage 13 - Core VM Crates (mostly complete)

**Completed**: 
- pleno-bytecode ✅
- pleno-compiler ✅
- pleno-vm ✅
- pleno-value ✅
- pleno-gc ✅

**Remaining**:
- pleno-common: Currently spans only, could expand to shared utilities
- pleno-diagnostics: Basic error reporting, could improve messages

---

## Stage 14 - v1 Hardening

**Completed**:
- Linter: Comprehensive style rules and unused variable detection ✅
- Docs Tooling: API documentation ✅

**Remaining**:
- **Testing Expansion**: Add more integration tests
- **Diagnostics**: Improve compiler/VM error messages with source spans

**Deferred (post-v1)**:
- GC Robustness: Full tracing management
- Performance: Benchmarking suite
- NaN boxing optimization

---

## v1 Release Criteria

- [x] All language features implemented and tested
- [x] Standard library complete (JSON, I/O, HTTP, Path, Process, etc.)
- [x] Tooling functional (LSP, formatter, linter)
- [x] Documentation complete
- [ ] 200+ passing tests
- [ ] No critical bugs in issue tracker
