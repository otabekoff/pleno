## 12. Memory Management & Performance

### 12.1 Ownership Hints (Optional)

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

### 12.2 Lazy Evaluation

```pleno
// Lazy evaluation
let lazy lazyValue = expensiveComputation();
// Computed only when accessed
print(lazyValue);  // Computation happens here

// Lazy sequences
let lazySeq = (0..)
    .map(|x| => x * x)
    .filter(|x| => x % 2 == 0)
    .take(10);  // Only computes 10 items
```

### 12.3 Memory Pooling

```pleno
// Object pooling
@pooled(size: 100)
class GameObject {
    // Instances recycled from pool
}
```