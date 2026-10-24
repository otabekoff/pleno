# Memory Management & Performance

Pleno manages memory automatically. Optional annotations let the compiler check how values are passed.

## Memory Model

- **Reference counting**: Heap values (arrays, maps, objects, closures) are reference counted.
- **Garbage collection**: A mark-based collector frees reference cycles. `std:gc` exposes it.
- **Value semantics**: Primitives are copied by value.

## Ownership Hints (Optional)

Pleno supports optional ownership annotations. The compiler checks them; they do not change how values are stored at runtime.

```pleno
// Move: the caller cannot use the value afterwards
fn takeOwnership(value: move String) {
    print("got " + value);
}

// Borrow: the parameter cannot be reassigned
fn borrowValue(value: &String) {
    print(value.length);
}

// Mutable borrow: the caller must pass a `let mut` binding
fn borrowMutable(value: &mut String) {
    print(value + "!");
}

let name = "Ada";
takeOwnership(name);
// print(name);         // compile error: use of moved value

borrowValue("Ada");      // 3

let mut label = "Ada";
borrowMutable(label);    // Ada!
// borrowMutable(name); // compile error: cannot take mutable reference to immutable binding
```

Assigning to a `&mut` parameter does not change the caller's variable.

## Lazy Evaluation

`let lazy` defers computation until the value is first used. Later uses reuse the result.

```pleno
fn computeExpensiveValue(): Int {
    print("computing");
    return 42;
}

let lazy expensiveResult = computeExpensiveValue();
print("before");
print(expensiveResult);  // prints "computing", then 42
print(expensiveResult);  // 42, without computing again
```

Ranges have no `map`, `filter` or `take` methods. To take items from an unbounded range, loop and `break`:

```pleno
let mut squares = [];
for (x in 0..) {
    if (squares.length == 5) { break; }
    if (x % 2 == 0) {
        squares.push(x * x);
    }
}
print(squares);  // [0, 4, 16, 36, 64]
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

// Allocated as usual for now
let obj = GameObject();
```

## Const & Immutability

Deep immutability prevents accidental mutations:

```pleno
const deep config = {
    "host": "localhost",
    "ports": [8080, 8443]
};

try {
    config.host = "remote";
} catch (e) {
    print(e.message);  // cannot modify immutable map
}

try {
    config.ports.push(9000);
} catch (e) {
    print(e.message);  // cannot modify immutable array
}
```

## Performance Tips

1. **Use `const`** for data that won't change
2. **Use `let lazy`** for expensive values that may not be needed
3. **Avoid closures in hot loops** when possible
4. **Batch I/O operations** instead of many small ones
