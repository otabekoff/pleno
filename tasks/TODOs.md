
## Priority: Standard Library & GC Integration

See `TECHNICAL_DEBT.md` for the full list of technical debt items.

Expand test suite and error messages

How our programming language reports errors, shows exact positions and tracing, backtracing? Ideal and standart solution? How error reporting when it happens works?

Implement date and time stdlib, and also we need something related to time stdlib and more stdlibs???

And continue with next tasks in TECHNICAL_DEBT.md and ROADMAP.md and if you need any info , specs.md file is ready for you. And make sure to update the files at the end of the jobs those are done.

----

## OLD PART
# Technical Debt

This document tracks all known technical debts, incomplete features, and areas requiring improvement in the Pleno project.

## Table of Contents
- [Standard Library](#standard-library)
- [CLI Tooling](#cli-tooling)
- [Formatter](#formatter)
- [Parser & AST](#parser--ast)
- [Compiler](#compiler)
- [Linter](#linter)
- [Error Handling](#error-handling)
- [Memory Management](#memory-management)
- [Test Coverage](#test-coverage)
- [Performance](#performance)
- [Documentation](#documentation)

---

## Standard Library

**Location:** `sources/std/`



### HTTP Module (`sources/std/http.pln`):
- **Line 5**: `HttpGet()` - TODO: Implement HTTP GET
- **Line 10**: `HttpPost()` - TODO: Implement HTTP POST
- **Line 15**: `HttpPut()` - TODO: Implement HTTP PUT
- **Line 20**: `HttpDelete()` - TODO: Implement HTTP DELETE

### Math Module (`sources/std/math.pln`):
- **Line 60**: Placeholder implementation for complex cases
- **Line 94**: Placeholder implementation
- **Line 98**: Returns placeholder value (0.5)

**Priority:** High  
**Effort:** Very High

---

## CLI Tooling

**Location:** `crates/pleno-cli/src/main.rs`

### Format Command:
1. **Line 138**: TODO: Implement actual formatting (indentation, spacing, etc.)
   - Currently only verifies parsing, doesn't actually format files
   - Message: "File {} is valid (formatting not yet fully implemented)"

2. **Line 149**: TODO: Recursively format all .pln files
   - Directory formatting not implemented
   - Message: "Directory formatting not yet fully implemented"

### Lint Command:
1. **Line 195**: TODO: Add more linting rules
   - Currently only basic checks, needs comprehensive rules

2. **Line 216**: TODO: Recursively lint all .pln files
   - Directory linting not implemented
   - Message: "Directory linting not yet fully implemented"

**Priority:** Medium  
**Effort:** Medium

---

## Formatter

**Location:** `crates/pleno-formatter/`

### AST Limitations (from ignored tests):
1. **Parentheses not preserved** - `test_complex_expressions` ignored
   - Formatter cannot re-add parentheses because AST doesn't store original parentheses
   - Would require AST changes

2. **Comments not preserved** - `test_comments_preservation` ignored
   - Comments are not part of the AST
   - Would require AST changes to include comments

3. **Blank lines not preserved** - `test_line_break_preservation` ignored
   - Blank lines between statements are not preserved in AST
   - Would require AST changes

### Parser Limitations:
5. **Nested classes not supported** - `test_nested_classes` ignored
   - Parser doesn't support nested class declarations

6. **Underscore parameters not supported** - `test_underscore_parameter_formatting` ignored
   - Underscore parameters in function declarations are not supported by parser

7. **Module blocks not supported** - `test_module_declaration_formatting` ignored
   - Parser expects `module Name;` not `module Name { ... }`

**Priority:** Medium  
**Effort:** High (requires AST/parser changes)

---

## Parser & AST

**Location:** `crates/pleno-parser/src/lib.rs`, `crates/pleno-ast/src/lib.rs`

### Missing Features:
3. **Nested classes** - Parser doesn't support class declarations inside classes
4. **Underscore parameters** - Parser expects `Ident` for parameters, not `Underscore`
5. **Module blocks** - Parser only supports `module Name;`, not `module Name { ... }`
6. **Comments in AST** - Comments are not preserved in AST nodes
7. **Parentheses in expressions** - Original parentheses grouping is lost

**Priority:** Medium  
**Effort:** High

---

## Compiler

**Location:** `crates/pleno-compiler/src/lib.rs`

### Error Handling:
1. **Line 238, 1294, 1328, 1410**: Uses `expect()` for loop stack operations
   - `let loop_ctx = self.pop_loop().expect("loop stack underflow");`
   - Should use proper error handling instead of panicking

2. **Line 1894**: Uses `expect()` for scope access
   - `let scope = self.locals.last_mut().expect("scope exists");`
   - Should use proper error handling

3. **Line 867, 890**: Uses `unreachable!()` for placeholder handling
   - Should be properly handled or removed

**Priority:** Medium  
**Effort:** Low

---

## Linter

**Location:** `crates/pleno-linter/src/lib.rs`, `crates/pleno-linter/tests/linter_tests.rs`

### Issues:
1. **Placeholder line/column numbers** - Line 183-184
   - Uses placeholder line/column numbers (0, 0)
   - Should calculate actual source positions

2. **Limited rule coverage** - Multiple TODO comments in tests
   - Missing semicolon detection
   - Missing spacing rules
   - Missing comprehensive style rules
   - Missing best practices checks
   - Missing unused variable detection

**Priority:** Medium  
**Effort:** Medium

---

## Error Handling

**Location:** Multiple files

### Issues:
1. **Excessive use of `unwrap()` and `expect()`** in tests
   - Found 50+ instances in test files
   - Should use proper error handling

2. **Panic usage in tests** - `crates/pleno-vm/tests/stdlib_tests.rs`
   - Multiple `panic!()` calls for unexpected types
   - Should use assertions or proper error handling

3. **Unreachable code** - Multiple `unreachable!()` calls
   - Should be properly handled or removed

**Priority:** Low  
**Effort:** Low

---

## Memory Management

**Location:** `crates/pleno-gc/`, `crates/pleno-vm/`

### Issues:
1. **GC not integrated** - From ROADMAP.md Stage 12
   - Mark-sweep GC exists in `pleno-gc` crate but not integrated into VM
   - VM still uses `Rc<RefCell<>>` for memory management
   - No automatic garbage collection

2. **Memory leaks potential** - Without GC, circular references could cause leaks

**Priority:** High  
**Effort:** Very High

---

## Test Coverage

**Location:** All test files

### Issues:
1. **Limited test coverage** - From ROADMAP.md Stage 14
   - Currently 53+ tests passing
   - Need more comprehensive coverage
   - Missing edge cases
   - Missing error path testing

2. **Ignored tests** - 8 formatter tests ignored due to missing features
   - Should be implemented or removed

3. **Placeholder test notes** - Multiple tests have "Note: ... may not work as expected"
   - Should be fixed or removed

**Priority:** Medium  
**Effort:** High

---

## Performance

**Location:** `crates/pleno-vm/`, `crates/pleno-compiler/`

### Issues:
1. **No performance optimizations** - From ROADMAP.md Stage 12
   - No benchmarking infrastructure
   - No performance profiling
   - No optimization passes in compiler

2. **No performance benchmarks** - From ROADMAP.md Stage 14
   - Need to establish baseline performance
   - Need to identify bottlenecks

**Priority:** Low  
**Effort:** High

---

## Documentation

**Location:** `docs/`, `README.md`

### Issues:
1. **Incomplete documentation** - From ROADMAP.md Stage 14
   - API documentation incomplete
   - Missing examples
   - Missing architecture documentation

2. **No docs tooling** - From ROADMAP.md Stage 11
   - No documentation generation
   - No API docs extraction

**Priority:** Low  
**Effort:** Medium

---

## Roadmap Items (From ROADMAP.md)

### Stage 8 - Functional Features (partial)
- Missing: tuple patterns in match expressions
- Missing: full pattern matching runtime verification
- Missing: advanced pattern matching features

### Stage 9 - Async + Concurrency (not started)
- Missing: actual async execution
- Missing: promise resolution
- Missing: event loop
- Missing: concurrency primitives

### Stage 10 - Standard Library (partial)
- Missing: full implementations of all stdlib functions
- Missing: native function bindings for system operations

### Stage 11 - Tooling (partial)
- Missing: full formatter implementation (indentation, spacing rules, code style)
- Missing: full linter implementation (comprehensive style rules, best practices, unused variable detection)
- Missing: docs tooling (generation, API docs)

### Stage 12 - Runtime + VM (partial)
- Missing: GC integration
- Missing: performance optimizations
- Missing: memory management improvements

### Stage 13 - Core VM Crates (partial)
- Partial: pleno-common (spans only)
- Partial: pleno-diagnostics (basic error reporting)
- Partial: pleno-value (primitives/collections/functions/classes)
- Partial: pleno-gc (exists but not integrated)

### Stage 14 - v1 Hardening (not started)
- Missing: test suite expansion
- Missing: error messages and diagnostics polish
- Missing: CLI parity with specs
- Missing: performance benchmarking and optimization
- Missing: documentation completion

---

## Summary

**Total Issues Identified:** 50+

**Priority Breakdown:**
- **High Priority:** 15 issues (REPL, VM methods, Stdlib, GC)
- **Medium Priority:** 20 issues (CLI, Formatter, Parser, Linter, Tests)
- **Low Priority:** 15+ issues (Error handling, Performance, Documentation)

**Estimated Effort:**
- **Very High Effort:** Stdlib implementation, GC integration
- **High Effort:** VM methods, Formatter AST changes, Parser features
- **Medium Effort:** REPL improvements, CLI features, Linter rules
- **Low Effort:** Error handling improvements, Test cleanup

---

## Next Steps

1. **Start with High Priority, Low/Medium Effort items:**
   - REPL improvements (exit handling, keyboard navigation)
   - Compiler error handling improvements
   - Linter placeholder fixes

2. **Then tackle High Priority, High Effort items:**
   - VM array methods (map, reduce, filter)
   - GC integration
   - Standard library implementations

3. **Finally address Medium/Low Priority items:**
   - Formatter AST improvements
   - Parser feature additions
   - Performance optimizations
   - Documentation completion