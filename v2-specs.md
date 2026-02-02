# Pleno V2 Specification (Draft)

This document outlines the proposed changes and new features for Pleno V2. The focus is on **Stability**, **Completeness**, **Interoperability**, and **Developer Experience**.

## 1. Syntax Enhancements

### 1.1 Optional Chaining (`?.`)

Safe access to nested properties and methods without explicit null checks.

```pleno
let street = user?.profile?.address?.street;
let result = object?.method?();
```

### 1.2 Nullish Coalescing (`??`)

Provide a default value when an expression evaluates to `null`.

```pleno
let timeout = config.timeout ?? 3000;
```

### 1.3 Piping Improvements

Enhanced support for the pipeline operator with placeholder support.

```pleno
let result = data
    |> process(_, "mode")
    |> validate();
```

## 2. Type System Evolution

### 2.1 Refined Generics

- **Variance**: Support for covariance (`out T`) and contravariance (`in T`).
- **Generalized Constraints**: More complex `where` clauses for type parameters.

### 2.2 Nominal vs Structural Typing

Introduced strict nominal typing for certain class hierarchies to improve safety in large codebases.

## 3. Interoperability (FFI)

Pleno V2 will support a Foreign Function Interface to call C and Rust libraries directly.

```pleno
import { FFI } from "std:ffi";

let libc = FFI.load("libc.so.6");
let puts = libc.bind("puts", [Type.String], Type.Int);
puts("Hello from C!");
```

## 4. Performance & Runtime

### 4.1 JIT (Just-In-Time) Compilation

A baseline JIT compiler targeting x86_64 and ARM64 to optimize hot loops.

### 4.2 Optimized Garbage Collection

- **Generational GC**: Faster collection for short-lived objects.
- **Concurrent Marking**: Reduce "stop-the-world" pauses.

## 5. Standard Library Expansion

- **`std:crypto`**: Standard primitives for SHA, AES, and JWT.
- **`std:db`**: A unified database interface (DBI) with native drivers for SQLite and PostgreSQL.
- **`std:grpc`**: First-class support for gRPC and Protobuf.
- **`std:ui`**: A platform-agnostic UI toolkit for building TUI and basic Desktop apps.

## 6. Stability & Tooling

### 6.1 Integrity Checks

- **Fuzzing Engine**: Integrated into `plm test --fuzz`.
- **Property Testing**: Native support for `proptest` style verification.

### 6.2 Professional Tooling

- **Language Server (LSP)**: Advanced refactoring, go-to-definition, and real-time type checking.
- **Debugger (DAP)**: Full step-through debugging support in VS Code.

---

_Note: This is a living document and will be refined as development progresses._
