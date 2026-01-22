# Pleno v1 Roadmap

This roadmap stages the implementation from core language to v1 features.

## Stage 0 - Workspace + Tooling (complete)

- Workspace layout, core crates, CLI skeleton
- Basic lexer/parser scaffolding
- Documentation baseline (mdBook)

## Stage 1 - Core Syntax + Expressions (complete)

- Completed: tokens for literals/idents/operators; operator precedence; expression AST/spans; UTF-8 decision; `&&`/`||` parsing + bytecode; dot token + parsing (member access only for map/object fields); range parsing + Range values; Char literal support; optional types (Int?); union types (Int | String); null coalescing (??) and safe navigation (?.) operators

## Stage 2 - Statements + Blocks (complete)

- Completed: `let`/`const`/assignments; scoped blocks; `if/else`, `while`, `break`, `continue`; `for` loops over ranges/arrays/map keys/sets (including multiple identifiers `for (k, v in coll)`)
- Missing: `for` over iterators/generators

## Stage 3 - Functions (complete)

- Completed: native/builtin calls; user-defined function declarations/calls; `return`; closures with capture; defaults/named args (including method/closure calls); variadics; keyword-only params; richer call semantics (partials/decorators infrastructure)
- Missing: keyword-only defaults with namespacing

## Stage 4 - Types (Gradual) (complete)

- Completed: type annotations parsing for parameters and return types; runtime primitive checking (`Int`, `String`, etc.) in `let`/`assign` and `catch`; class type checking in `is` operator; optional types (Int?); union types (Int | String); Void and Any type support

## Stage 5 - Modules + Imports (complete)

- Completed: `import` statement (module load/execute), module export map, module caching, module declarations, structured imports/exports (string paths), basic resolver for relative paths and `sources/`, package/registry integration (stub), resolver for std/registry schemes
- Missing: full package manager

## Stage 6 - Data Structures (complete)

- Completed: map/array/set literals; array/map indexing; index assignment; array/map/set iteration; set membership; tuples (positional and named); tuple index access; dot access for Strings, Arrays, Sets, and Maps (builtin methods like `.len`, `.upper()`, `.lower()`, `.trim()`, `.split()`, `.contains()`, `.startsWith()`, `.endsWith()`, `.replace()`, `.push()`, `.pop()`, `.clear()`, `.slice()`, `.reverse()`, `.indexOf()`, etc.); array slicing syntax ([1..4], [2..], [..3]); array slice() method; array map/filter/reduce method stubs (partial - need VM function calling support)

## Stage 7 - OOP Core (complete)

## Stage 9 - Functional Features (partial)

- Completed: pipeline operator, basic pattern matching, lambdas/closures (shorthand and anonymous fn expressions)
- Partial: 
- Missing: 


## Stage 10 - Error Handling (complete)

- Completed: basic try/catch/finally, throw syntax and VM support; multiple catch blocks with type checking (e.g., `catch (e: MyError)`); Result and Option type constructors (Ok, Err, Some, None); Result methods (unwrap, unwrapOr, expect, isOk, isErr); Option methods (unwrap, unwrapOr, expect, isSome, isNone)

## Stage 11 - Async + Concurrency (not started)

- Not started: async/await, promises

## Stage 12 - Standard Library Stubs (partial)

- Completed: basic stdlib module stubs (std:core, std:io, std:json, std:http, std:math) with function signatures
- Missing: actual implementations of stdlib functions

## Stage 13 - Tooling (partial)

- Completed: LSP MVP (diagnostics, go-to-def); basic formatter CLI command (pleno format); basic linter CLI command (pleno lint); type checker CLI command (pleno check)
- Missing: full formatter implementation (indentation, spacing rules); full linter implementation (style rules, best practices); docs tooling

## Stage 14 - Runtime + VM (Register-Based) (partial)

- Completed: register bytecode format; compiler for literals/unary/binary/if/while/assignments; VM arithmetic/comparison/jumps; function call frames, class instances; exception handling
- Missing: GC baseline (mark-sweep), performance work

## Stage 14.1 - Core VM Crates (partial)

- Completed: pleno-bytecode, pleno-compiler, pleno-vm
- Partial: pleno-common (spans only), pleno-diagnostics (basic), pleno-value (primitives/collections/functions/classes)

## Stage 15 - v1 Hardening

- Test suite expansion
- Error messages, diagnostics polish
- CLI parity with specs
