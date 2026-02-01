# Pleno v1: Final Hardening & Minor Tasks

This document tracks minor technical debts, deferred features, and final polish tasks for the Pleno v1 release.

## Technical Debt & Polish
- [ ] **Diagnostics**: Add even more suggestions/hints to common parser errors.
- [ ] **Standard Library**: Complete any remaining undocumented methods in `std:io` and `std:net`.
- [ ] **Performance**: Investigate potential optimizations in the VM's dispatch loop for `match` expressions.
- [x] **Linter**: Add a rule for detecting shadowed variables that might cause confusion.
- [ ] **Compiler**: Refine constant folding for complex data structures (nested arrays/maps).

## Documentation
- [x] **Tutorials**: Create a "Getting Started" guide for new users.
- [ ] **Standard Library Reference**: Generate a full API reference from doc comments.
- [x] **Examples**: Add more comprehensive examples of async/concurrency patterns.

## Infrastructure
- [ ] **CI/CD**: Ensure all integration tests run on every push.
- [x] **Benchmarks**: Add regression testing for performance benchmarks.

## Future (v1.x)
- [ ] **JIT**: Detailed JIT implementation plan.
- [ ] **Macro expansion explorer**: Tooling to see how macros expand.
- [ ] **Package Registry**: Design for a central Pleno package registry.
