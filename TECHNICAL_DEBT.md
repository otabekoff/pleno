# Technical Debt

This document tracks known technical debts and areas requiring improvement in Pleno.

---

## High Priority

### Standard Library
**Location:** `sources/std/`

**Math Module** (`math.pln`): Some placeholder implementations remain

**Effort:** Medium

---

### Memory Management
**Location:** `crates/pleno-gc/`, `crates/pleno-vm/`

- Mark-sweep GC exists in `pleno-gc` but not integrated into VM
- VM still uses `Rc<RefCell<>>` for memory management
- Potential memory leaks from circular references

**Effort:** Very High

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
- Comments in AST
- Parentheses preservation

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
1. REPL improvements (exit handling, keyboard navigation) ✅
2. VM array methods (map, reduce, filter) ✅
3. Lexer comment support ✅
4. GC integration
5. Standard library implementations (Math, Regex, universal concatenation) ✅



