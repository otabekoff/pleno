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
