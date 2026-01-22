# Pleno v1 Roadmap

This roadmap stages the implementation from core language to v1 features.

## Stage 0 - Workspace + Tooling (complete)

- Workspace layout, core crates, CLI skeleton
- Basic lexer/parser scaffolding
- Documentation baseline (mdBook)

## Stage 1 - Core Syntax + Expressions (partial)

- Completed: tokens for literals/idents/operators; operator precedence; expression AST/spans; UTF-8 decision; `&&`/`||` parsing + bytecode
- Partial: dot token + parsing (member access only for map/object fields); range parsing + Range values (iteration semantics handled in Stage 2)
- Missing: full member access semantics (methods/classes), more literal types

## Stage 2 - Statements + Blocks (partial)

- Completed: `let`/`const`/assignments; scoped blocks; `if/else`, `while`, `break`, `continue`
- Partial: `for` loops over ranges/arrays/map keys/sets (unbounded `start..` requires `break`)
- Missing: `for` over iterators/generators

## Stage 3 - Functions (partial)

- Completed: native/builtin calls; user-defined function declarations/calls; `return`; closures with capture
- Partial: first-class functions (locals/returns supported)
- Missing: defaults/named args

## Stage 4 - Types (Gradual) (not started)

- Not started: type annotations, primitive checking, optional/union types

## Stage 5 - Modules + Imports (not started)

- Not started: module declarations, import syntax, resolver

## Stage 6 - Data Structures (partial)

- Completed: map/array/set literals; array/map indexing; index assignment; array/map/set iteration; set membership
- Partial: dot access only on map/object fields
- Missing: tuples, richer collection APIs

## Stage 7 - OOP Core (partial)

- Completed: class declarations with methods/constructors; instance fields via dot; `this`; `super` + inheritance; static methods
- Partial: access modifiers (parsed, not enforced)
- Missing: static fields, access control rules

## Stage 8 - Advanced OOP (not started)

- Not started: inheritance/interfaces/abstracts, properties/static members

## Stage 9 - Functional Features (not started)

- Not started: lambdas/closures, pipeline operator, pattern matching

## Stage 10 - Error Handling (not started)

- Not started: try/catch/finally, Result/Option syntax

## Stage 11 - Async + Concurrency (not started)

- Not started: async/await, promises

## Stage 12 - Standard Library Stubs (not started)

- Not started: stdlib stubs (string/math/collections/IO/JSON)

## Stage 13 - Tooling (partial)

- Completed: LSP MVP (diagnostics, go-to-def)
- Missing: formatter, linter, docs tooling

## Stage 14 - Runtime + VM (Register-Based) (partial)

- Completed: register bytecode format; compiler for literals/unary/binary/if/while/assignments; VM arithmetic/comparison/jumps
- Partial: function call frames, class instances
- Missing: GC baseline (mark-sweep), performance work

## Stage 14.1 - Core VM Crates (partial)

- Completed: pleno-bytecode, pleno-compiler, pleno-vm
- Partial: pleno-common (spans only), pleno-diagnostics (basic), pleno-value (primitives/collections/functions/classes)

## Stage 15 - v1 Hardening

- Test suite expansion
- Error messages, diagnostics polish
- CLI parity with specs
