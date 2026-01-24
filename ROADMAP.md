# Pleno v1 Roadmap

This roadmap tracks remaining implementation work for v1.

---

## Stage 12 - Runtime + VM (Complete)

- [x] Core VM implementation
- Performance optimizations (deferred post-v1)
- NaN boxing (deferred post-v1)

---

## Stage 13 - Core VM Crates (Complete)

- [x] pleno-value, pleno-bytecode, pleno-compiler, pleno-vm

---

## Stage 14 - v1 Hardening (Complete)

**Completed**:
- [x] GC Robustness: Cyclic reference reclamation verified
- [x] Benchmarking suite (`std:benchmark`) implemented
- [x] Testing expansion: **102 tests passing**

**Deferred (post-v1)**:
- NaN boxing optimization

---

## v1 Release Criteria

- [x] All language features implemented and tested
- [x] Standard library complete (JSON, I/O, HTTP, Path, Process, etc.)
- [x] Tooling functional (LSP, formatter, linter)
- [x] Documentation complete
- [x] **102 passing tests** (exceeds 100+ goal)
- [ ] No critical bugs in issue tracker
