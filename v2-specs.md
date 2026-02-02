# Pleno V2 Specification (Draft)

This document outlines the formal technical specifications for Pleno V2.

## 1. Syntax & Expressions

### 1.1 Optional Chaining (`?.`)

Safe access to nested properties and methods. Returns `null` if any part of the chain is `null`.

```pleno
let street = user?.profile?.address?.street;
let value = list?.[0];
let result = obj?.method?();
```

### 1.2 Nullish Coalescing (`??`)

Returns the right-hand operand when the left-hand operand is `null`.

```pleno
let timeout = config.timeout ?? 3000;
```

### 1.3 Control Flow Expressions

`if` and `match` are now full expressions that return values.

```pleno
let status = if (authenticated) "Welcome" else "Login Required";

let result = match (command) {
    "start" => 1,
    "stop" => 0,
    _ => -1
};
```

## 2. Type System Evolution

### 2.1 Generics: Variance

Pleno V2 introduces variance annotations for generic type parameters.

- **Covariance (`out T`)**: Allows a type to be treated as a more general type. Useful for producer-only interfaces.
- **Contravariance (`in T`)**: Allows a type to be treated as a more specific type. Useful for consumer-only interfaces.

```pleno
interface Producer<out T> {
    fn produce(): T;
}

interface Consumer<in T> {
    fn consume(item: T): Void;
}
```

---
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

---

### 2.2 Reified Runtime Types

Type parameters are no longer strictly erased. The VM can access type information at runtime for metadata and validation.

## 3. Advanced OOP Features

### 3.1 Operator Overloading

Classes can define custom behavior for standard operators.

| Operator | Method Name   |
| -------- | ------------- |
| `+`      | `operator +`  |
| `-`      | `operator -`  |
| `*`      | `operator *`  |
| `/`      | `operator /`  |
| `==`     | `operator ==` |
| `[]`     | `operator []` |

Example:

```pleno
class Complex {
    re: Float; im: Float;

    operator +(other: Complex): Complex {
        return Complex(this.re + other.re, this.im + other.im);
    }
}
```

### 3.2 Custom Decorators

Functions marked with `@decorator` can wrap other functions or classes at compile time.

```pleno
@decorator
fn benchmark(target: Function): Function {
    return fn(...args) => {
        let start = Time.now();
        let result = target(...args);
        print("Elapsed: " + (Time.now() - start));
        return result;
    };
}
```

## 4. Performance & Memory

### 4.1 Ownership Hints

Optional annotations for performance-critical logic.

- `move`: Transfer ownership, preventing further use in the caller.
- `ref`: Immutable reference (borrowing).
- `ref mut`: Mutable reference.

### 4.2 Arena & Pooling

- `@pooled`: Class annotation for object pooling.
- `Arena`: Manual memory management handles for high-throughput batches.

## 5. Standard Library Highlights

### 5.1 `std:crypto`

Primitives for SHA256, HMAC, and AES.

```pleno
import { SHA256 } from "std:crypto";
let hash = SHA256.hash("secret");
```

### 5.2 `std:db`

Unified interface for relational databases. Supports SQLite and PostgreSQL backends.

### 5.3 `std:http` (Full)

High-performance asynchronous server with Middleware support.


---

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
---

_Note: This is a living draft and subject to changes based on implementation feasibility._
