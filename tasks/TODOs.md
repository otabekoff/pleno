
## Priority: Standard Library & GC Integration

- [x] Memory management (breaking Rc cycles)
- [x] Concurrency primitives (Mutex, Channels)
- [x] spawn() function improvements

See `TECHNICAL_DEBT.md` for the full list of technical debt items.

Expand test suite and error messages

How our programming language reports errors, shows exact positions and tracing, backtracing? Ideal and standart solution? How error reporting when it happens works?

Implement date and time stdlib, and also we need something related to time stdlib and more stdlibs???

Continue with the implementation/fix/bugfix/update of next tasks in TECHNICAL_DEBT.md and ROADMAP.md and if you need any info , specs.md file is ready for you. And make sure to update the files at the end of the jobs those are done.

In vscode only highlighting working. When imported modules, ctrl + click is not opening that part of / the file. Also on hovering nothing is being shown on code of .pln. No icon of extension and files of .pln. Also, there is no README for better support.

Add vscode snippets.

Does each crate is doing their job only. For example stdlib crate should be responsilble for stdlib, but the compile crate may be trying to be dominant and having the stdlib functions inside of it. Is it a good practice? I think no. What do you think? This is one of the example of the thing I wanted to explain you. There might be so many of others. So, find and fix them.

We had a solid specs.md, then while developing based on the specs.md we may have introduced many additional things too. But they are not in the specs.md, I've also divided the specs.md into docs/specs folder into files, you may update those files to be correct. For example not all of the stlibs may not be documented all other data structures we have or there are many things not documented, now it is time to document them all.
---

To add new languages to Highlight.js officially, you need to develop a separate 3rd-party language module, package it as a standalone file (like
highlight.langName.min.js), and contribute it to the highlightjs/highlight.js GitHub organization or host it yourself, following their guidelines; the core team encourages community contributions for new languages rather than developing them in-house, using their Language Definition Guide and Contributor Checklist for packaging. 
How to Add a New Language

    Develop the Language Module:
        Refer to the Highlight.js Language Definition Guide to learn how to define a new language's syntax.
        Implement the language grammar, often in a separate file like highlight.bicep.js.
    Package it as a 3rd-Party Module:
        Follow the project's Language Contributor Checklist for packaging your module correctly.
        You'll create a build (e.g., highlight.langName.min.js) that includes your language.
    Contribute or Host:
        Option A (Recommended by Project): Submit your language as a separate repository to the highlightjs GitHub organization for official linking and potential inclusion in discovery.
        Option B: Host the file yourself and provide a link to it (e.g., via a CDN).
    Integrate into Your Project:
        Include the specific language script in your HTML or import it in Node.js (e.g., <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/latex.min.js"></script>).
        Use hljs.registerLanguage('your-lang-name', yourLangModule) before calling hljs.highlightAll() or hljs.highlightElement() to load it. 

Key Point: The Highlight.js core team focuses on the library's core, not new languages, so community-driven 3rd-party modules are the standard way new languages are supported. 

AND


To add support for a new language to highlight.js and consequently to VS Code, you must
create and maintain a third-party language grammar module, as highlight.js no longer merges new language grammars into its core library. VS Code will recognize this new language via its own extension system. 
Contributing to Highlight.js
The highlight.js core team encourages community contributions of third-party language grammars, and is willing to link to or even host them within the highlightjs GitHub organization, provided the contributor is willing to maintain them. 

    Develop a Language Definition: Create a new repository for your language grammar. The language definition file is a function that returns a language object with keywords, modes, and regexps.
    Add Metadata and Tests: Include specific metadata comments at the top of your language file and provide code examples in the test/detect/<language>/ directory to help with testing and demo purposes.
    Package as a Module: Follow the guidelines in the extra/3RD_PARTY_QUICK_START.md file within the highlight.js repository to properly package your definition as a third-party module.
    Publish and Request Listing: Publish your package (e.g., on npm) and then submit a pull request to the main highlight.js repository to add your language to the SUPPORTED_LANGUAGES.md list. 

Adding to VS Code
VS Code provides language support through extensions. The language support you create for highlight.js will not automatically appear in VS Code. 

    Create a VS Code Extension: To get your language support officially recognized and easily installable in VS Code, you need to develop a dedicated VS Code extension. You can use the Yeoman generator for a basic template.
    Define Syntax Highlighting: Inside the extension, you'll define the language's syntax, often using a TextMate grammar (.tmLanguage.json file), which can be based on your highlight.js work.
    Publish to Marketplace: Once your extension is working, you can publish it to the Visual Studio Code Marketplace for users to install. 

By following these separate processes for both highlight.js and VS Code, you can ensure your new language receives broad support.


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
1. **GC integrated** ✅
2. **Breaking Rc cycles** ✅

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