# Technical Debt

This document tracks known technical debts and areas requiring improvement in Pleno.

---

## Resolved ✅

### Compiler Error Handling
**Location:** `crates/pleno-compiler/src/lib.rs`
- Resolved `expect()` calls ✅
- Resolved unused `Result` warnings ✅
- Fixed unreachable pattern warnings ✅

### Linter
**Location:** `crates/pleno-linter/`
- AST spans for accurate line/column numbers ✅
- Unused variable detection with usage tracking ✅
- Style rules (operator spacing, line length, trailing spaces) ✅

### Documentation
- API documentation for all stdlib modules ✅
- VitePress-based docs infrastructure ✅

### CLI Tooling
**Location:** `crates/pleno-cli/`
- Format command with AST-based formatting ✅
- Directory formatting ✅
- Linting with comprehensive rules ✅

---

## Medium Priority

### Formatter Limitations
**Location:** `crates/pleno-formatter/`

Requires AST changes:
- Parentheses not preserved in expressions
- Comments not preserved
- Blank lines not preserved

**Effort:** High (requires parser changes)

---

### Parser & AST
**Location:** `crates/pleno-parser/`, `crates/pleno-ast/`

Missing features:
- Module blocks (`module Name { ... }`)
- Comments in AST
- Parentheses preservation
- Array type syntax (`Type[]`)

**Note:** AST spans implemented across all nodes ✅

**Effort:** High

---

## Low Priority

### Memory Management
**Location:** `crates/pleno-gc/`, `crates/pleno-vm/`

- GC roots integrated in VM loop ✅
- `GcRc` wrapping `Rc<RefCell<>>` for memory management
- Cannot break circular references in pure `Rc` chains

**Effort:** High (deferred to post-v1)

---

### Error Handling in Tests
- Excessive `unwrap()` and `expect()` usage (cosmetic)
- Can be addressed incrementally

**Effort:** Low

---

### Test Coverage
- Currently 190+ tests passing
- 8 formatter tests ignored (missing features)
- Need more integration tests

**Effort:** Medium

---

### Performance
- No benchmarking infrastructure
- No optimization passes

**Effort:** High (deferred to post-v1)

---

## Summary

| Priority | Status |
|----------|--------|
| Resolved | Compiler, Linter, Documentation, CLI |
| Medium | Formatter, Parser/AST |
| Low/Deferred | GC, Performance, Test coverage |

**v1 Focus:**
1. Add more integration tests
2. Improve error messages
3. Fix any critical bugs
