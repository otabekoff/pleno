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

## 7. Features Deferred from V1

These features were specified in V1 but deferred due to scope constraints:

### 7.1 Operator Overloading

User-defined operators for custom types.

```pleno
class Vector {
    x: Float;
    y: Float;

    operator +(other: Vector): Vector {
        return Vector(this.x + other.x, this.y + other.y);
    }

    operator ==(other: Vector): Bool {
        return this.x == other.x && this.y == other.y;
    }
}
```

### 7.2 Package Manager (PLM)

Full package management with registry support.

```bash
plm init my-project
plm add lodash@1.0.0
plm install
plm publish
```

### 7.3 Ownership Hints

Optional Rust-like borrow/move semantics for performance optimization.

```pleno
fn transfer(move data: Buffer) { ... }
fn read(ref data: &Buffer) { ... }
fn update(ref mut data: &mut Buffer) { ... }
```

### 7.4 Memory Pooling

Object pooling for high-frequency allocations.

```pleno
@pooled(size: 100)
class Particle {
    x: Float; y: Float; velocity: Float;
}
```

### 7.5 Custom Decorator Definitions

Allow users to define custom decorators.

```pleno
@decorator
fn logged(target: Function): Function {
    return fn(...args) => {
        print("Calling: " + target.name);
        return target(...args);
    };
}
```

### 7.6 HTTP Server (Full Implementation)

Real TCP listener for `Server.listen()`.

```pleno
let server = Server(router);
server.listen(8080, fn() => print("Listening on :8080"));
```

---

_Note: This is a living document and will be refined as development progresses._
