# Pleno v1 Roadmap

This roadmap stages the implementation from core language to v1 features.

## Stage 0 - Workspace + Tooling (complete)

- Workspace layout, core crates, CLI skeleton
- Basic lexer/parser scaffolding
- Documentation baseline (mdBook)

## Stage 1 - Core Syntax + Expressions (partial)

- Completed: tokens for literals/idents/operators; operator precedence; expression AST/spans; UTF-8 decision; `&&`/`||` parsing + bytecode; dot token + parsing (member access only for map/object fields); range parsing + Range values
- Missing: full member access semantics (methods/classes), more literal types

## Stage 2 - Statements + Blocks (complete)

- Completed: `let`/`const`/assignments; scoped blocks; `if/else`, `while`, `break`, `continue`; `for` loops over ranges/arrays/map keys/sets (including multiple identifiers `for (k, v in coll)`)
- Missing: `for` over iterators/generators

## Stage 3 - Functions (complete)

- Completed: native/builtin calls; user-defined function declarations/calls; `return`; closures with capture; defaults/named args (including method/closure calls); variadics; keyword-only params; richer call semantics (partials/decorators infrastructure)
- Missing: keyword-only defaults with namespacing

## Stage 4 - Types (Gradual) (partial)

- Completed: type annotations parsing for parameters and return types
- Not started: primitive checking, optional/union types

## Stage 5 - Modules + Imports (complete)

- Completed: `import` statement (module load/execute), module export map, module caching, module declarations, structured imports/exports (string paths), basic resolver for relative paths and `sources/`, package/registry integration (stub), resolver for std/registry schemes
- Missing: full package manager

## Stage 6 - Data Structures (partial)

- Completed: map/array/set literals; array/map indexing; index assignment; array/map/set iteration; set membership; tuples (positional and named); tuple index access
- Partial: dot access only on map/object fields
- Missing: richer collection APIs

## Stage 7 - OOP Core (complete)

## Stage 9 - Functional Features (partial)

- Completed: pipeline operator, basic pattern matching, lambdas/closures (shorthand and anonymous fn expressions)
- Partial: 
- Missing: 


## Stage 10 - Error Handling (partial)

- Completed: basic try/catch/finally, throw syntax and VM support
- Missing: Result/Option syntax, multiple catch blocks with type checking

## Stage 11 - Async + Concurrency (not started)

- Not started: async/await, promises

## Stage 12 - Standard Library Stubs (not started)

- Not started: stdlib stubs (string/math/collections/IO/JSON)

## Stage 13 - Tooling (partial)

- Completed: LSP MVP (diagnostics, go-to-def)
- Missing: formatter, linter, docs tooling

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
