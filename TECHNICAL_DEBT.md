# Technical Debt

This document tracks known technical debts and areas requiring improvement in Pleno.

---

## Medium Priority

### Memory Management
**Location:** `crates/pleno-gc/`, `crates/pleno-vm/`
- Circular reference handling in `GcRc` chains (currently `Rc` based fallback)
- Full tracing GC management

### Test Coverage
- Reach 100% core language feature coverage
- Need more integration tests for async/concurrency

---

## Low Priority

### Error Handling in Tests
- Excessive `unwrap()` and `expect!()` usage (cosmetic)
- Can be addressed incrementally

### Performance
- No benchmarking infrastructure
- No optimization passes in compiler
- NaN boxing optimization

---

## Summary

| Priority | Status |
|----------|--------|
| High | Completed (OOP/PM/Diagnostics) |
| Medium | GC, Test coverage |
| Low/Deferred | Performance, Test cleanup |

**v1 Focus:**
1. Reach Release Criteria (250+ tests)
