# Pleno v1 Roadmap

This roadmap tracks remaining implementation work for v1.

---

## Stage 12 - Runtime + VM (mostly complete)

**Remaining**:
- Performance optimizations (optional for v1)
- NaN boxing (optional for v1)

---

## Stage 13 - Core VM Crates (mostly complete)

**Remaining**:
- pleno-common: Currently spans only, could expand to shared utilities

---

## Stage 14 - v1 Hardening

**Remaining**:
- **Testing Expansion**: Add more integration tests

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
