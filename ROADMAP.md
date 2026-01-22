# Pleno v1 Roadmap

This roadmap stages the implementation from core language to v1 features.

## Stage 0 - Workspace + Tooling (complete)

- Workspace layout, core crates, CLI skeleton
- Basic lexer/parser scaffolding
- Documentation baseline (mdBook)

## Stage 1 - Core Syntax + Expressions (partial)

- Tokens for literals, identifiers, operators
- Operator precedence parsing (unary, binary)
- Expression AST and spans
- UTF-8 string model decision (Unicode scalars by default)
- Logical operators (`&&`, `||`) parsing and bytecode emission
- Reserved keywords added for `class`, `for`, `this`, `super`, `return` (parsing pending)
- Dot token added for member access (parsing + map property access implemented)
- Range tokens (`..`, `..=`) and parsing (compiler/VM for numeric ranges)
- Prefix/suffix ranges parsed and compiled to Range values
- Array literal tokens (`[ ]`) and parsing

## Stage 2 - Statements + Blocks (partial)

- `let`, `const`, assignments (mutable enforcement)
- Blocks `{ ... }` with scoped locals
- `if/else`, `while`, `break`, `continue`
- `for` loops over numeric ranges (including `..end` and `start..`), arrays, map keys, and sets

## Stage 3 - Functions (not started)

- Function declarations + calls
- Parameters, defaults, named args
- Return statements

## Stage 4 - Types (Gradual) (not started)

- Type annotations on vars/params/returns
- Basic type checking for primitives
- Optional and union types (syntax + AST)

## Stage 5 - Modules + Imports (not started)

- Module declarations
- `import` syntax + resolver

## Stage 6 - Data Structures (partial)

- Map literals with string keys
- Map property access via dot (maps only)
- Array literals and indexing
- Map indexing with string keys
- Array iteration in `for` loops
- Map key iteration in `for` loops
- Set literals (initial)
- Set membership via index
- Set iteration in `for` loops
- Object literal fields via map syntax (used for member access)
- Arrays, maps, sets, tuples, ranges
- Indexing and member access

## Stage 7 - OOP Core (not started)

- Classes, fields, methods
- `this`, constructors
- Access modifiers

## Stage 8 - Advanced OOP (not started)

- Inheritance, interfaces, abstract classes
- Properties, static members

## Stage 9 - Functional Features (not started)

- Lambdas, closures
- Pipeline operator
- Pattern matching

## Stage 10 - Error Handling (not started)

- `try/catch/finally`
- `Result` and `Option` syntax

## Stage 11 - Async + Concurrency (not started)

- `async`/`await`, promises

## Stage 12 - Standard Library Stubs (not started)

- String (UTF-8, normalization), math, collections, IO, JSON

## Stage 13 - Tooling (partial)

- Formatter, linter, docs stub
- LSP server MVP (diagnostics, go-to-def)

## Stage 14 - Runtime + VM (Register-Based) (partial)

- Register-based bytecode format
- Compiler (AST -> bytecode) for literals, unary/binary, if/while, assignments
- VM execution engine for arithmetic/comparison/jumps
- GC baseline (mark-sweep) (pending)

## Stage 14.1 - Core VM Crates (partial)

- pleno-common (spans, interning, utilities) (spans only)
- pleno-diagnostics (errors, labels, reporting) (basic diagnostic)
- pleno-value (runtime values) (primitives, ranges, arrays, maps, sets)
- pleno-bytecode (instruction set, chunk)
- pleno-compiler (AST -> bytecode)
- pleno-vm (executor)

## Stage 15 - v1 Hardening

- Test suite expansion
- Error messages, diagnostics polish
- CLI parity with specs
