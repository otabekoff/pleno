# Technical Debt

This document tracks known technical debts and areas requiring improvement in Pleno.

---

## High Priority



### Memory Management
**Location:** `crates/pleno-gc/`, `crates/pleno-vm/`

- GC roots (registers, globals, call frames, async tasks, promises) integrated in VM loop
- VM still uses `GcRc` (wrapping `Rc<RefCell<>>`) for memory management
- GC can collect unreachable objects from `heap`, but cannot yet break circular references purely within `Rc` chains

**Effort:** High

---

## Medium Priority

### CLI Tooling
**Location:** `crates/pleno-cli/src/main.rs`

- Format command uses `pleno-formatter` for robust AST-based formatting ✅
- Directory formatting supported in CLI ✅
- Linting needs more comprehensive rules

**Effort:** Medium

---

### Formatter Limitations
**Location:** `crates/pleno-formatter/`

Requires AST changes:
- Parentheses not preserved in expressions
- Comments not preserved
- Blank lines not preserved

Parser limitations:
- Module blocks not supported

**Effort:** High

---

### Parser & AST
**Location:** `crates/pleno-parser/`, `crates/pleno-ast/`

Missing:
- Nested classes ✅
- Underscore parameters ✅
- Module blocks (`module Name { ... }`)
- Comments in AST (Spans implemented, capture logic needed)
- Parentheses preservation (Spans implemented, capture logic needed)
- Array type syntax (`Type[]`) in function signatures (currently requires `Array<Type>`)
- **Note:** AST now supports Spans across all nodes (Refactor completed Jan 2026).

**Effort:** High

---

### Compiler Error Handling
**Location:** `crates/pleno-compiler/src/lib.rs`

- Resolved many `expect()` calls in `pleno-compiler` ✅
- Resolved all unused `Result` warnings in `pleno-compiler` ✅
- Fixed unreachable pattern warning in `pleno-vm` ✅

**Effort:** Low

---

### Linter
**Location:** `crates/pleno-linter/`

- Uses placeholder line/column numbers (0, 0)
- Limited rule coverage (missing semicolon, spacing, style, best practices, unused variable detection)

**Effort:** Medium

---

## Low Priority

### Error Handling in Tests
- Excessive use of `unwrap()` and `expect()` in tests (50+ instances)
- Multiple `panic!()` calls for unexpected types
- Multiple `unreachable!()` calls

**Effort:** Low

---

### Test Coverage
- **Expand test suite**: Currently 53+ tests passing, need significantly more coverage.
- **Error Messages**: Improve compiler and runtime error messages (e.g. `invalid operands for +`).
- 8 formatter tests ignored due to missing features
- Some tests have "may not work as expected" notes

**Effort:** High

---

### Performance
- No benchmarking infrastructure
- No performance profiling
- No optimization passes in compiler

**Effort:** High

---

### Documentation
- API documentation incomplete
- Missing examples and architecture documentation
- No docs tooling (generation, API docs)

**Effort:** Medium

---

## Summary

| Priority | Count | Notes |
|----------|-------|-------|
| High | 4 | REPL, VM methods, Stdlib, GC |
| Medium | 5 | CLI, Formatter, Parser, Compiler, Linter |
| Low | 4 | Error handling, Tests, Performance, Documentation |

**Next Steps:**
1. GC: Investigate breaking `Rc` cycles or transitioning to full tracing management
2. Async: Implement `Promise.all` and `spawn()`
3. Standard Library: Expand `String` and `Map` methods
4. Linter: Implement comprehensive rules



