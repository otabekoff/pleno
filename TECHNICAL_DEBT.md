# Technical Debt

This document tracks known technical debts and areas requiring improvement in Pleno.

---

## Resolved ✅

### Parser & AST
**Location:** `crates/pleno-parser/`, `crates/pleno-ast/`
- Module blocks (`module Name { ... }`) ✅
- Comments in AST and Doc comments (`///`) ✅
- Parentheses preservation ✅

### Formatter
**Location:** `crates/pleno-formatter/`
- Parentheses preserved in expressions ✅
- Comments preserved and interleaved ✅
- Blank lines preserved (partially via comment logic) ✅

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

### Error Messages
- **Improved Diagnostics**: Compiler now emits rich errors with source code snippets and carets ✅
- **Location**: `pleno-compiler` uses `pleno-diagnostics`

### Documentation
- API documentation for all stdlib modules ✅
- VitePress-based docs infrastructure ✅

### Generics
- Implemented parsing and AST support ✅
- Implemented type erasure in compiler ✅
- Implemented formatting support ✅

### CLI Tooling
**Location:** `crates/pleno-cli/`
- Format command with AST-based formatting ✅
- Directory formatting ✅
- Linting with comprehensive rules ✅

---

### OOP & Control Flow
- `abstract` classes and methods implemented in AST/Parser/Formatter ✅
- `interface` and `implements` implemented in AST/Parser/Formatter ✅
- `do-while` loops fully implemented (AST, Parser, Formatter, Compiler, VM execution handled by existing jumps) ✅
- Labeled `break` and `continue` fully implemented (AST, Parser, Formatter, Compiler resolution) ✅
- Property getters/setters syntax implemented (AST, Parser, Formatter) ✅

### Typing & Deep Immutability
- Array type syntax (`Type[]`) ✅
- Function type syntax (`(T1, T2) => R`) ✅
- Union types (`T1 | T2`) and Optional types (`T?`) in AST/Parser ✅
- `const deep` (Deep Immutability) syntax implemented in AST/Parser/Formatter ✅

---

## High Priority

### Missing Spec Implementations
**Location:** `crates/pleno-compiler`, `crates/pleno-vm`, `crates/pleno-cli`

Features documented in `specs.md` but missing in codebase:
- Runtime enforcement for `@pure` functions
- Runtime enforcement for `const deep` immutability ✅
- Runtime error catchability (e.g. out-of-bounds, immutability violations) ✅
- Bytecode emission for property getters/setters ✅
- Runtime instantiation checks for `abstract` classes ✅
- Interface satisfaction validation at compile-time/runtime ✅
- Property getters/setters syntax support (Lexer/Parser/Formatter ✅)

### Package Management Completion
- Complete implementation of `pleno install` (fetch/clone)
- Proper dependency resolution

---

## Medium Priority

### Memory Management
**Location:** `crates/pleno-gc/`, `crates/pleno-vm/`
- Circular reference handling in `GcRc` chains (currently `Rc` based fallback)
- Full tracing GC management

### Test Coverage
- Currently 200+ tests passing
- Reach 100% core language feature coverage
- Need more integration tests for async/concurrency

---

## Low Priority

### Error Handling in Tests
- Excessive `unwrap()` and `expect()` usage (cosmetic)
- Can be addressed incrementally

### Performance
- No benchmarking infrastructure
- No optimization passes in compiler
- NaN boxing optimization

---

## Summary

| Priority | Status |
|----------|--------|
| Resolved | Compiler, Linter, Documentation, CLI, Diagnostics, Generics, Formatter, Parser (partially) |
| High | OOP (abstract/interface), Typing (union/array/fn), PM |
| Medium | GC, Test coverage |
| Low/Deferred | Performance, Test cleanup |

**v1 Focus:**
1. Complete core language features (OOP/Typing)
2. Finalize Package Management
3. Reach Release Criteria (250+ tests)
