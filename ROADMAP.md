# Pleno v1 Roadmap

This roadmap stages the remaining implementation work for v1.

---

## Stage 8 - Functional Features (partial)

**Completed**: pipeline operator, basic pattern matching (identity/wildcard patterns), lambdas/closures (shorthand and anonymous fn expressions), Tuple patterns in match expressions, Full pattern matching runtime verification

**Remaining**:
- Advanced pattern matching features

---

## Stage 9 - Async + Concurrency (partial)

**Completed**: async/await execution, promise resolution, event loop, output buffering (flushing), hidden promise ID handling; Concurrency primitives (Mutex, Channels) ✅; spawn() function ✅

---

## Stage 10 - Standard Library (partial)

**Completed**: basic stdlib module stubs; JSON module fully implemented; I/O module (files/dirs) fully implemented; HTTP module fully implemented; Math and Regex libraries aligned with specs; Native function bindings for system operations (OS module); Time Standard Library (DateTime, Duration) ✅

**Remaining**:
- Advanced collection methods (Map/Set comprehensive API)
- Standard library documentation generation

---

## Stage 11 - Tooling (partial)

**Completed**: LSP MVP (diagnostics, go-to-def); basic formatter/linter CLI commands; type checker CLI command; optional types (`Int?`) and union types (`Int | String`) runtime type checking; Full formatter implementation in CLI

**Remaining**:
- Full linter implementation (comprehensive style rules, best practices, unused variable detection)
- Docs tooling (generation, API docs)

---

## Stage 12 - Runtime + VM (partial)

**Completed**: register bytecode format; compiler for all language features; VM arithmetic/comparison/jumps; function call frames, class instances; exception handling; array methods (map, filter, reduce); 175+ passing tests; universal string concatenation; collection length aliases; memory management (breaking Rc cycles) ✅

**Remaining**:
- Performance optimizations
- NaN boxing (optional)

---

## Stage 13 - Core VM Crates (partial)

**Completed**: pleno-bytecode, pleno-compiler, pleno-vm (fully functional)

**Partial**:
- pleno-common (spans only)
- pleno-diagnostics (basic error reporting)
- pleno-value (primitives/collections/functions/classes) ✅
- pleno-gc (fully integrated) ✅

---

## Stage 14 - v1 Hardening (not started)

- **GC Robustness**: Break `Rc` cycles or transition to full tracing management
- **Linter Completion**: Comprehensive style rules and unused variable detection
- **Testing Expansion**: Increase coverage from 53+ tests to full feature parity
- **Diagnostics**: Improve compiler/VM error messages and source spans
- **Docs Tooling**: Automated API documentation generation
- **Performance**: Benchmarking suite and optimization passes
