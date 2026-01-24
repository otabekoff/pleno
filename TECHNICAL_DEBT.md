# Technical Debt

This document tracks known technical debts and areas requiring improvement in Pleno.

---

## Post-v1 Improvements

### Standard Library
- [x] Implement `std:http` (Client) using `reqwest`
- [x] Implement `std:regex` using `regex` crate
- Implement `std:test` assertion framework (currently ad-hoc in tests)

### Language Features
- **Generics**: Currently parsed but not fully enforced/reified in runtime.
- **Interfaces**: Parsed but conformance checks are basic.
- **Async/Await**: Basic Promise support exists; needs full event loop integration.

### Spec Compliance Gaps (Found during verification)
- **If Expressions**: `let x = if (...) ...` syntax is not currently supported by parser (expects statement).
- **Closures**: Runtime error observed in closure variable capture test.
- **Field Declarations**: Parser requires delimiters/types more strictly than spec implies (e.g. `name;` vs `name: String;`).
- **Type Checking**: Strict type checking might be too aggressive or reporting errors confusingly (`expected Int, found Int`).

### Tooling
- Error messages: Improve span reporting and suggestions.
- LSP: Complete implementation for IDE support.

---

## Summary

| Priority | Status |
|----------|--------|
| High | **Completed** (Core VM, Parser, Essential StdLib, Enums, VM Refactoring) |
| Medium | **Completed** (HTTP Client, Regex) |
| Low | Optimization, Further Refactoring |

**v1 Status**: Feature Complete (Essential Scope)
