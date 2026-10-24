# Pleno v1 Roadmap

This roadmap tracks remaining implementation work for v1.


## v1 Release Criteria

**STATUS**: RELEASE CANDIDATE 1

---

## Vision Alignment: "Type-Safe Dynamic Language"

Current status against the 2026 Modern Model:

| Feature              | Status         | Implementation Details                                                            |
| :------------------- | :------------- | :-------------------------------------------------------------------------------- |
| **Dynamic**          | ✅ Implemented | No type annotations required (`let x = 5`).                                       |
| **Type-Safe**        | ✅ Implemented | Runtime checks emit `LoadNull` / `Instanceof` checks. Memory safe via Rust.       |
| **Gradually Typed**  | ✅ Implemented | Optional types (`let x: Int = 5`) enforced at runtime.                            |
| **Runtime Verified** | ✅ Implemented | Variable reassignment and upvalue captures are type-checked at runtime.          |
| **JIT / AOT**        | ⏸️ Deferred    | Bytecode interpreter only. JIT/AOT deferred to post-v1.                           |
| **IDE Inference**    | ⚠️ Partial     | LSP provides static analysis, but no runtime-profile feedback yet.                |
| **Zero UB**          | ✅ Implemented | Safe Rust + VM checks (e.g., stack limits, strict operations).                    |
