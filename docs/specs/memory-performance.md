# Memory Management & Performance

Pleno manages memory automatically. Optional annotations let the compiler check how values are passed.

## Memory Model

This section describes the current implementation, not a guarantee; it can change in any release.

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

A parameter holds its own copy of the value (for an array, map or object, its own copy of the reference), so a function cannot rebind the caller's variable. `&mut` says the function may change the value **in place**: `items.push(4)` on a `&mut Int[]` parameter is visible to the caller. Reassigning a `&mut` parameter (`value = ...`, `value += ...`, `value++`) would only change the copy, so it is a compile error for every type:

```pleno
fn addItem(items: &mut Int[]) {
    items.push(4);          // changes the caller's array
}

let mut numbers = [1, 2, 3];
addItem(numbers);
print(numbers.length);      // 4

// fn shout(text: &mut String) { text = text + "!"; }
// compile error: cannot assign to '&mut' parameter 'text': the caller's variable
// would not change; mutate the value in place or return the new value
```

Strings, numbers, booleans and tuples cannot change in place, so for them return the new value instead: `fn shout(text: String): String { return text + "!"; }` and `label = shout(label);`.

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

These are suggestions for the current implementation:

1. **Use `const`** for data that won't change
2. **Use `let lazy`** for expensive values that may not be needed
3. **Avoid closures in hot loops** when possible
4. **Batch I/O operations** instead of many small ones
