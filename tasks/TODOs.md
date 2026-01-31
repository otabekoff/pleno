
## Priority: Standard Library & GC Integration

http://localhost:5173/pleno/specs/ is showing next page to introduction which is itself, and causing loop on docs.

i++ ++i else if finallies

after doing output files creation, remove them, clean the project. or don't create at the project root. instead log texts should be in .logs folder.

Debug mode, production mode logs for development.
Remove debug logs.


Now, as we have finished v1 of the Pleno programming language, we fully support specs.md and many other things not listed in it too. Now what I want is, you need to add those already implemented, but missed things in specs.md to be noted.

Always do cargo clippy --workspace --all-targets --all-features -- -D warnings and github add    commit and push. gh?

Code Runner extension should support for Pleno programming language.

After doing each job, do cargo fmt for workspace.

Now, as we have finished v1 of the Pleno programming langguage, I want you to fully think and write, plan the next v2 of Pleno programming language. We need V2 roadmap, v2-specs.md. Here we'll add better features, make the old and inconsistent things enhanced, better, implement new features and support many things, more stdlibs. and many more. Think more and write detailed, Things will be added, upgraded, updated, fixed, supported.

We have to deploy v1 fully. Now we need to update the docs.

LSP and IDE features. REPL, also colors?.

Better error reporting like Rust's.

More comment types?

end, sep and others?

V1 missing things noti implemented, deferred, technical debts should be identifed and should be saved to one file. v1-last-tasks.md

Scripts, management tools, makefile better vesrion. Organize the project.

Move to new version of taks management in tasks folder. New workflow.

More examples. Benchmark. IDE Features.



Clean the project sources folder.

We've to think about bad practices, inconsistencies. We've to review what we have already and reconsider everything. Need to find inconsistencies, duplicated, bad dx and practices and fix and solve and upgrade, update them.

We've to think about better github management, better actions, better commits, better tools of github, better branch management.

New folder called prompts.

V2, weill be the most ideal one.

Add more licenses from rust like githuib stuff.

Also, do we have support for more than one print stuff parameters like in python  wiith comma and other programming languages? Endless? Look, should we add it or our programmign language design is different and has such thing support in another way? Or better solution? What about println? What about sep, end and other such things to print? Think about this and should we add them?

I heard somewhere, making everything Boxed or boexed expressions in programming language is not good way and it makes slow. Is it true? Why? How can we avoid it? Is it possible to avoid it? How can we make it better? Do we have such cases or not? If we have such places, do we need to make them better and avoid or are there the places where they should be like so?

See `TECHNICAL_DEBT.md` for the full list of technical debt items. Update it. Now we need to search the project, the comments, and find all leakages, unimplemented stuffs, and write all the technical debts in technical debt file.

make the project clean and organized. Of course mainly project root. And review README.md and make it a programming language README for open source.

Review CODE_OF_CONDUCT, CONTRIBUTING< LICENSE, SECURITY markdown fiels too. We need ARCHITECTURE and such files too.

Also update README.dev.md.

Let's have new roadmap for V2. 

Let's review whole the docs and make it great.

To release production ready v1, what should we do?

To earn at a stage where we can write compiler of Pleno with itself, what should we do?

Run becnchmarks.

Expand test suite and error messages

Continue with the implementation/fix/bugfix/update of next tasks in TECHNICAL_DEBT.md and ROADMAP.md and if you need any info , specs.md file is ready for you. And make sure to update the files at the end of the jobs those are done. And if completely done tasks and if we don't address them anymore, remove them from ROADMAP.md and TECHNICAL_DEBT.md.

Ok continue with @SPECS_LOG.md  and if you need you can use @specs.md  for reference

Check problems tab

How we can optimize the code?

How we can make Pleno very fast even if it is dynamic?

Does each crate finished its work fully and complete? I mean, any crate may not be giving errors or even warnings and working minimalistically, but not to be complete yet supporitng everything needed for v1, not compatible with specs and not including everthing needed, so identify limitations and the blocks and make their list and begin working on them.

AI can do the things, and make the project work finally without errors. But while developing, it can make so many mistakes with logic. Many beginner codes, no optimization, and very needless codes too. So, we've to avoid them and make the project code professional and written with best practices.  AI does children like coding sometimes, sometimes like professionals and sometimes like juniors or interns or someone who doesn't know coding. So, let's find those kind of unprofessional parts and enhance.

Via Claude AI generate V2 specs. Upgrades/updates/news/critical and breakign changes, new features, inclrementals, bugfixing, avoiding inconsistencies and more. Instead of fixing the real problems, or being lazy to implement and refactor, AI just might wanted to lie or be lazy or be tricky and just satisfied error and warning linters and compiler. So, let's find them and actually fix those parts.

AI model's generation exceeded the maximum output token limit like error might happen and couldn't do some works, or execution tool may have failed in some cases and some changes might not have been applied.

Instruction to AI:
Eyes are coward, hands are heroes. The eye is afraid, but the hand is brave. Fear looks bigger than it really is. It’s not as hard as it looks. Once you start, it’s easier. The hardest part is getting started. Something looks scary or difficult at first, but when you actually start doing it, it turns out to be manageable. And the thing you're escaping will be the thing that you need in most cases. So, don't escape from implementing, refactring and making great results.

Add benching, reporting. Pleno vs Python vs JS vs TS vs Java vs C++ vs C vs Rust vs Kotlin. Speed and other tests? Builtin time tracking tests vs outer command execution time tests. Languages' inner time trackers may differ, so we need some command prompt/terminal tool?

To reach self-hosted compiler programming level what we need to do?

To make this programming language very fast what we need to do?

To make this programming language very stable what we need to do?

To make this programming language complete what we need to do?

To make this programming language all platforms supported and very good tooling what we need to do?

Now it is time to renew/fefresh the specs.md with what our programming language has and can do.

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