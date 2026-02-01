# Pleno Programming Language
## Complete Language Specification v1.0

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices

## 19. Performance Features

### 19.1 JIT Compilation
- Hot path optimization
- Automatic inlining of small functions
- Dead code elimination

### 19.2 Memory Management
- Generational garbage collection
- Reference counting for immediate cleanup
- Optional manual memory hints
- Object pooling support


### Stage 12 - Runtime + VM (Done)

- **Performance optimizations**: **Missing**.

### 19.3 Optimization Hints

```pleno
// Inline hint
@inline
fn fastOperation(x: Int): Int {
    return x * 2;
}

// No optimization (for debugging)
@no_optimize
fn debugFunction() {
    // Keep unoptimized for debugging
}

// Hot path hint
@hot
fn criticalPath() {
    // Prioritize optimization
}
```

---

[Performance Features Debt](./tasks/performance_features_debt.md)

## 18. Performance

- [ ] 19.1 JIT (Deferred)
- [ ] 19.2 Memory Hints (Deferred)

## 19. Security

- [ ] 20.2 Sandboxing (Deferred)


# Security Features Technical Debt

## Summary
Audited specs.md section 20 (Security Features) against implementation. Completed granular IO and network access control. Remaining items deferred to future cycles.

## Status by Feature

### 20.2 Sandboxing
| Feature | Status | Notes |
|---------|--------|-------|
| OS-level sandboxing | ❌ DEFERRED | Needs seccomp/unshare (linux) |
| Memory usage quotas | ❌ DEFERRED | Per-module isolation |

## Future Recommendations

1. **Defer to future**:
   - Full OS-level process sandboxing (using `seccomp` or similar).
   - Memory usage quotas per-module (advanced isolation).
   - Secure random number generator (std:crypto).


# Performance Features Technical Debt

## Summary
Audited specs.md section 19 (Performance Features) against actual implementation.

## Status by Feature

### 19.1 JIT Compilation
| Feature | Status | Notes |
|---------|--------|-------|
| Hot path optimization | ❌ NOT IMPLEMENTED | No JIT compiler exists |
| Automatic inlining | ❌ NOT IMPLEMENTED | No inlining pass |
| Dead code elimination | ✅ IMPLEMENTED | Constant boolean conditions in if statements |

### 19.2 Memory Management
| Feature | Status | Notes |
|---------|--------|-------|
| Generational GC | ❌ NOT IMPLEMENTED | Only Mark-and-Sweep in `pleno-value/src/gc.rs` |
| Reference counting | ✅ IMPLEMENTED | Uses `Rc<RefCell<T>>` wrapper (`GcRc`) |
| Manual memory hints | ✅ IMPLEMENTED | `GC.collect()`, `GC.heapSize()`, `GC.isEnabled()` in stdlib |
| Object pooling | ❌ NOT IMPLEMENTED | No pool support |

### 19.3 Optimization Hints
| Annotation | Status | Notes |
|------------|--------|-------|
| `@pure` | ✅ IMPLEMENTED | Compiler checks `is_pure` flag |
| `@inline` | ✅ IMPLEMENTED | Stored in `FunctionProto.is_inline` |
| `@no_optimize` | ✅ IMPLEMENTED | Stored in `FunctionProto.no_optimize` |
| `@hot` | ✅ IMPLEMENTED | Stored in `FunctionProto.is_hot` |

## Existing Implementation

### GC (`pleno-value/src/gc.rs`)
- `MarkAndSweepGC` - basic mark-and-sweep collector
- `GcRc<T>` - reference-counted wrapper
- `GCManager` - allocation threshold-based collection triggers
- Collection triggered when `current_allocation >= allocation_threshold`

### Annotations (`pleno-compiler/src/lib.rs`)
- `@pure` - sets `is_pure` flag on functions, checked at runtime
- Other annotations are parsed into `RuntimeAnnotation` structs but not used for optimization

## Recommendations

1. **Remove from v1 specs** or **mark as "Future"**:
   - JIT compilation (major undertaking)
   - Generational GC (complex implementation)
   - Object pooling

2. **Could implement for v1**:
   - `@inline` hint for small functions
   - Dead code elimination in compiler
   - `@no_optimize` for debugging

3. **Already working**:
   - `@pure` annotation
   - Basic Mark-and-Sweep GC
   - Reference counting via `Rc`




----


And make a framework like express in nodejs.

LLVM or cranelift support.

----


# Pleno v1: Final Hardening & Minor Tasks

This document tracks minor technical debts, deferred features, and final polish tasks for the Pleno v1 release.

## Future (v1.x)
- [ ] **JIT**: Detailed JIT implementation plan.
- [ ] **Switch Opcode**: Implement `Switch` instruction for O(1) match dispatch (result of performance investigation).
- [ ] **Macro expansion explorer**: Tooling to see how macros expand.
- [ ] **Package Registry**: Design for a central Pleno package registry.


---

csv, sqlite3 or latest

---

Unicode, emojis, gradients, pleno ascii text.


---

VSCode extension should be able to download and setup Pleno.

---


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
- **StdLib Design**: Standardize on Object-Oriented vs Module-based API. Current implementation is mixed (Module-based `std:time` vs Class-based `File`).
- **Documentation**: Verify all stdlib modules are fully documented (e.g. `std:io` buffer methods).

---

## Summary

| Priority | Status |
|----------|--------|
| High | **Completed** (Core VM, Parser, Essential StdLib, Enums, VM Refactoring) |
| Medium | **Completed** (HTTP Client, Regex) |
| Low | Optimization, Further Refactoring |

**v1 Status**: Feature Complete (Essential Scope)


---


