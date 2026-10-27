# Performance Features

::: info Implementation notes
This page describes what the current compiler and VM do. It is not part of the language reference or the [compatibility promise](./compatibility.md): optimizations can be added, changed or removed in any release, and a program must not depend on them for its results.
:::

### Optimizations

- **Dead code elimination**: removes unreachable code and unused bindings at compile time. Compile errors in that code are still reported.
- **Function inlining**: the optimizer can inline functions marked `@inline`.
- **Constant folding**: computes constant expressions such as `2 * 60` during compilation.

### Optimization hints

```pleno
// Let the optimizer inline calls to this function
@inline
fn fastOperation(x: Int): Int {
    return x * 2;
}

// Keep constant folding and dead code removal away from this function
@no_optimize
fn debugFunction() {
    let unused = 1 + 2;
}

// Recorded on the function; it has no effect today
@hot
fn criticalPath() {
}

print(fastOperation(21));   // 42
```

An inlined call behaves like the call: the arguments are evaluated once, before the body, the parameter and return annotations are checked (`TypeError: add returned String, expected Int`), and names in the body mean what they mean where the function is declared. Calls in top-level code are inlined. A call in a function body is inlined only when the function has no annotations, its body is `return` of an expression of its parameters, and the arguments are variables or literals. Every other call is a normal call, as are calls with named or spread arguments and calls to a function that is generic, `async`, has `move` or `&` parameters, or has a lambda, a nested declaration or `try` in its body.

For how the VM runs code, see [Bytecode VM](./bytecode_vm.md). Plans such as a JIT compiler and a generational garbage collector are in the [version 2 draft](https://github.com/otabekoff/pleno/blob/main/notes/v2-draft.md).
