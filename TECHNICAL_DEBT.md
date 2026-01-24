# Technical Debt

This document tracks known technical debts and areas requiring improvement in Pleno.

---

## Completed

### Memory Management
- [x] Circular reference handling (`collect_with` marking implemented)

### Test Coverage  
- [x] 102 tests passing (core language feature coverage complete)

### Performance
- [x] Benchmarking infrastructure (`std:benchmark` implemented)

---

## Remaining (Low Priority)

### Error Handling in Tests
- Excessive `unwrap()` and `expect!()` usage (cosmetic)
- Can be addressed incrementally

### Performance (Deferred)
- No optimization passes in compiler
- NaN boxing optimization

### Known Limitations
- `super` keyword not fully supported in constructors
- Set `has()` method not implemented
- Spread in function calls not fully supported
- Array/string slice syntax not supported (use `substring()`)

---

## Summary

| Priority | Status |
|----------|--------|
| High | Completed (OOP/PM/Diagnostics) |
| Medium | Completed (GC, Test coverage, Benchmarking) |
| Low/Deferred | Performance, Known limitations |

**v1 Status**: Ready for release candidate
