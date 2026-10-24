# std:benchmark

Performance measurement utilities for benchmarking code execution.

## Import

```pleno
import { Benchmark } from "std:benchmark";
```

## Usage

### `run(name: String, func: Function): Void`

Runs a named benchmark and prints timing results.

```pleno
Benchmark.run("Array creation", fn() {
    let arr = [];
    for (i in 0..10000) {
        arr.push(i);
    }
});
// Output: Array creation: 12.34ms
```

### `measure(func: Function): Float`

Measures execution time of a function and returns duration in milliseconds.

```pleno
let duration = Benchmark.measure(fn() {
    // Code to measure
    let mut sum = 0;
    for (i in 0..100000) {
        sum += i;
    }
});
print("Took " + duration.toString() + "ms");
```

## Example: Comparing Algorithms

```pleno
import { Benchmark } from "std:benchmark";

fn bubbleSort(arr) {
    // ... implementation
}

fn quickSort(arr) {
    // ... implementation
}

let data = [5, 2, 8, 1, 9, 3, 7, 4, 6];

Benchmark.run("Bubble Sort", fn() {
    bubbleSort(data.clone());
});

Benchmark.run("Quick Sort", fn() {
    quickSort(data.clone());
});
```

---
