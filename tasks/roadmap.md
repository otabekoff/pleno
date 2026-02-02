# Pleno v1 Roadmap

This roadmap tracks remaining implementation work for v1.

---

## Deferred Features (Post-v1)

The following features from `specs.md` are deferred to v1.x or v2:

- **Advanced Standard Library**: `std:security` (Sandboxing)
- **Advanced Types**: `Tuple`, `Decorator`, `Interface` (runtime checks), `Generics` (runtime reification)
- **Tooling**: Full LSP implementation, Formatter perfection
- **Performance**: JIT Compilation, NaN boxing

---

## v1 Release Criteria

- [x] **Concurrency Fix**: `spawn` is now truly asynchronous and leverages the VM task system.
- [x] **Functional Globals**: Implemented `map`, `filter`, and `reduce` in the global scope to support pipeline syntax.
- [x] **Math Module Flattening**: `std:math` functions are now accessible directly (e.g., `import { sqrt } from "std:math"`).
- [x] **VM Exception Safety**: Implemented `run_depths` to prevent frame underflow during exception unwinding.
- [x] Core language features implemented (including Enums, Classes, Functions)
- [x] Essential Standard Library (`io`, `json`, `process`, `path`, `time`)
- [x] Documentation complete
- [x] Stable pass rate on test suite

**STATUS**: RELEASE CANDIDATE 1

---

## Vision Alignment: "Type-Safe Dynamic Language"

Current status against the 2026 Modern Model:

| Feature              | Status         | Implementation Details                                                            |
| :------------------- | :------------- | :-------------------------------------------------------------------------------- |
| **Dynamic**          | ✅ Implemented | No type annotations required (`let x = 5`).                                       |
| **Type-Safe**        | ✅ Implemented | Runtime checks emit `LoadNull` / `Instanceof` checks. Memory safe via Rust.       |
| **Gradually Typed**  | ✅ Implemented | Optional types (`let x: Int = 5`) enforced at runtime.                            |
| **Runtime Verified** | ⚠️ Partial     | Variable reassignment verified, but **upvalue captures** currently bypass checks. |
| **JIT / AOT**        | ⏸️ Deferred    | Bytecode interpreter only. JIT/AOT deferred to post-v1.                           |
| **IDE Inference**    | ⚠️ Partial     | LSP provides static analysis, but no runtime-profile feedback yet.                |
| **Zero UB**          | ✅ Implemented | Safe Rust + VM checks (e.g., stack limits, strict operations).                    |
