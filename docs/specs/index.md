# Pleno Language Reference

This is the reference for the Pleno language as it is implemented, and the basis of the 1.0 specification. It describes only features that exist; what 1.x will keep stable is in the [compatibility promise](compatibility.md). Designs for later versions are in the [version 2 draft](https://github.com/otabekoff/pleno/blob/main/notes/v2-draft.md) and are not part of this reference.

**File Extension:** `.pln`  
**Paradigms:** object-oriented, functional and procedural  
**Typing:** dynamic, with optional type annotations that are checked when the code runs; `pleno check` reports some mismatches before running

## Overview

Pleno is a dynamically typed language with classes, interfaces, closures, pattern matching, generics, `async`/`await` and a standard library for files, processes, HTTP, data formats and testing. Programs run on a bytecode VM.

Every ```` ```pleno ```` example on these pages is run by the documentation tests, so it works as shown. The "Implementation notes" pages at the end of the sidebar describe the current implementation and plans; they are not part of the reference.

### Strings

Strings are UTF-8, in source files and at runtime. A position in a string counts Unicode characters (scalar values): `"héllo"[1]` is `"é"`.

`length`, `[i]`, slices such as `s[1..3]`, `indexOf`, `substring`, `charAt` and regex match positions all count characters the same way, so a position from one of them can be passed to another: `"héllo".length` is `5` and `"héllo".indexOf("l")` is `2`. A character is one scalar value, so an accent written as a separate combining mark (`e` followed by U+0301) counts as its own character. See [String operations](./standard-library.md#string-operations) for what each method does with positions out of range.

## Quick Reference

| Topic           | Link                                                       |
| --------------- | ---------------------------------------------------------- |
| Compatibility   | [What 1.x guarantees](compatibility.md)                    |
| Core Syntax     | [Variables, Types, Operators](core-syntax-fundamentals.md) |
| Control Flow    | [if/else, loops, match](control-flow.md)                   |
| Functions       | [fn, closures, async](functions.md)                        |
| OOP             | [Classes, Interfaces, Inheritance](OOP.md)                 |
| Data Structures | [Arrays, Maps, Sets](data-structures.md)                   |
| Error Handling  | [try/catch, Result, Option](error-handling.md)             |
| Modules         | [import/export](modules-imports.md)                        |
| Generics        | [Type Parameters, Variance](generics.md)                   |
| Async           | [async/await, Promises, Channels](async-concurrency.md)    |
