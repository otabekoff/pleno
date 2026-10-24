# Memory Management & Performance

Pleno uses automatic memory management with garbage collection, plus optional hints for performance optimization.

## Memory Model

- **Garbage Collection**: Automatic memory reclamation
- **Reference Counting**: RC for value types with cycle detection
- **Value Semantics**: Primitives copied by value

## Ownership Hints (Optional)

Pleno supports optional ownership annotations for performance-critical code:

```pleno
// Move semantics (transfer ownership)
fn takeOwnership(value: move String) {
    // value is moved, original is invalidated
}

// Borrow (read-only reference)
fn borrowValue(value: &String) {
    // value is borrowed, cannot be modified
}

// Mutable borrow
fn borrowMutable(value: &mut String) {
    // value can be modified
}
```

## Lazy Evaluation

Defer computation until the value is actually needed:

```pleno
// Lazy value - computed on first access
let lazy expensiveResult = computeExpensiveValue();
print(expensiveResult);  // Computation happens here

// Lazy sequences (infinite streams)
let squares = (0..)
    .map(fn(x) => x * x)
    .filter(fn(x) => x % 2 == 0)
    .take(10);  // Only computes 10 items
```

## Memory Pooling

Reuse object allocations for performance:

::: warning Planned
`@pooled` is accepted and recorded on the class, but allocation does not use a pool yet, and the annotation takes no arguments.
:::

```pleno
@pooled
class GameObject {
    public x: Float;
    public y: Float;

    fn reset() {
        this.x = 0;
        this.y = 0;
    }
}

// Objects are recycled from the pool
let obj = GameObject();  // May reuse existing allocation
```

## Const & Immutability

Deep immutability prevents accidental mutations:

```pleno
const deep config = {
    "host": "localhost",
    "ports": [8080, 8443]
};

// config.host = "remote";      // ERROR: Cannot mutate
// config.ports.push(9000);     // ERROR: Cannot mutate nested
```

## Performance Tips

1. **Use `const`** for data that won't change
2. **Prefer lazy evaluation** for expensive computations
3. **Use pooling** for frequently allocated objects
4. **Avoid closures in hot loops** when possible
5. **Batch I/O operations** instead of many small ones
