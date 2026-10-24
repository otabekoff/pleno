# std:benchmark

Performance measurement utilities for benchmarking code execution.

## Import

```pleno
import { Benchmark } from "std:benchmark";
```

`run` and `measure` can also be imported directly: `import { run, measure } from "std:benchmark"`.

## Usage

### `run(name: String, func: Function, iterations: Int = 1): Int`

Calls `func` `iterations` times, prints the average time per call, and returns it in nanoseconds.

```pleno
import { Benchmark } from "std:benchmark";

let ns = Benchmark.run("Array creation", fn() {
    let arr = [];
    for (i in 0..10000) {
        arr.push(i);
    }
}, 10);
// Array creation: <average> ns/iter (10 iterations)
```

### `measure(func: Function): Float`

Calls `func` once and returns how long it took, in milliseconds.

```pleno
import { Benchmark } from "std:benchmark";

let duration = Benchmark.measure(fn() {
    let mut sum = 0;
    for (i in 0..100000) {
        sum += i;
    }
});
print(`Took ${duration}ms`);
```

## Example: Comparing Algorithms

```pleno
import { Benchmark } from "std:benchmark";

fn insertionSort(input) {
    let arr = input.clone();
    for (i in 1..arr.length) {
        let key = arr[i];
        let mut j = i - 1;
        while (j >= 0 && arr[j] > key) {
            arr[j + 1] = arr[j];
            j -= 1;
        }
        arr[j + 1] = key;
    }
    return arr;
}

let data = [5, 2, 8, 1, 9, 3, 7, 4, 6];

Benchmark.run("Insertion sort", fn() {
    insertionSort(data);
}, 100);

Benchmark.run("Built-in sort", fn() {
    data.clone().sort();
}, 100);
```
