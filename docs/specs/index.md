# Pleno Programming Language

## Complete Language Specification v0.1.0 (Alpha)

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices

## Overview

Pleno is a modern programming language designed for clarity, safety, and expressiveness. It combines the best features from multiple programming paradigms while maintaining simplicity and consistency.

### Key Features

- **Type Safety**: Optional static typing with gradual typing support
- **Modern Syntax**: Clean, readable syntax inspired by modern languages
- **First-class Functions**: Functions as values, closures, lambdas
- **Pattern Matching**: Powerful pattern matching for control flow
- **Async/Await**: Built-in asynchronous programming support
- **Rich Standard Library**: Comprehensive stdlib for common tasks

### String Encoding Decision

Pleno uses UTF-8 for all string literals and runtime strings.

Key points:

- UTF-8 is the 2026+ industry standard across tools, platforms, and APIs.
- ASCII remains 1 byte per char, keeping source code compact.
- Full Unicode support is preserved for international text and emoji.

String indexing defaults to Unicode scalar values for safety. Byte-level access
is provided via explicit APIs.

## Quick Reference

| Topic           | Link                                                       |
| --------------- | ---------------------------------------------------------- |
| Core Syntax     | [Variables, Types, Operators](core-syntax-fundamentals.md) |
| Control Flow    | [if/else, loops, match](control-flow.md)                   |
| Functions       | [fn, closures, async](functions.md)                        |
| OOP             | [Classes, Interfaces, Inheritance](OOP.md)                 |
| Data Structures | [Arrays, Maps, Sets](data-structures.md)                   |
| Error Handling  | [try/catch, Result, Option](error-handling.md)             |
| Modules         | [import/export](modules-imports.md)                        |
| Generics        | [Type Parameters, Variance](generics.md)                   |
| Async           | [async/await, Promises, Channels](async-concurrency.md)    |
