# Pleno Programming Language
## Complete Language Specification v1.0

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices






[Performance Features Debt](./tasks/performance_features_debt.md)

## 18. Performance

- [ ] 19.1 JIT (Deferred)
- [ ] 19.2 Memory Hints (Deferred)

## 19. Security

- [ ] 20.2 Sandboxing (Deferred)
- [ ] 
   - JIT compilation (major undertaking)
   - Generational GC (complex implementation)
   - Object pooling
   - 
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

Code Runner extension should support for Pleno programming language.

---

LSP and IDE features. REPL, also colors?.

More comment types?


---

New folder called prompts.

---

V2, we'll be the most ideal one.

Add more licenses from rust like githuib stuff.

---

I heard somewhere, making everything Boxed or boexed expressions in programming language is not good way and it makes slow. Is it true? Why? How can we avoid it? Is it possible to avoid it? How can we make it better? Do we have such cases or not? If we have such places, do we need to make them better and avoid or are there the places where they should be like so?

---

To earn at a stage where we can write compiler of Pleno with itself, what should we do?

---

How we can optimize the code?

How we can make Pleno very fast even if it is dynamic?

---

# QUESTIONS
- To reach self-hosted compiler programming level what we need to do?
- To make this programming language very fast what we need to do?


---

## Deferred Features (Post-v1)

The following features from `specs.md` are deferred to v1.x or v2:

- **Advanced Standard Library**: `std:security` (Sandboxing)
- **Advanced Types**: `Tuple`, `Decorator`, `Interface` (runtime checks), `Generics` (runtime reification)
- **Tooling**: Full LSP implementation, Formatter perfection
- **Performance**: JIT Compilation, NaN boxing

----


Move to new version of taks management in tasks folder. New workflow.


---

We've to think about bad practices, inconsistencies. We've to review what we have already and reconsider everything. Need to find inconsistencies, duplicated, bad dx and practices and fix and solve and upgrade, update them.


---


## Priority: Standard Library & GC Integration

---

Pleno V2 will be made withh Rust too, but not connected to Rust. Meaning we'll use Rust to make Pleno v2, but it won't be connected to Rust. Meaning it works even if there is no Rust installed on the system. Pleno needs to reach a level where we can write its compiler with itself. Will have self-hosted compiler and at the end we'll get rid of Rust. Pleno will become independent programming language.