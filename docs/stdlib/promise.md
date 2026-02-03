# std:promise

Promise and async utilities for concurrent programming.

## Import

```pleno
import { Promise, spawn } from "std:promise";
```

## Creating Promises

### `Promise(executor: Function): Promise`

Creates a new Promise with a resolver/rejecter pattern.

```pleno
let promise = Promise(fn(resolve, reject) {
    // Async operation
    if (success) {
        resolve(result);
    } else {
        reject("Error occurred");
    }
});
```

## Chaining

### `then(onFulfilled: Function): Promise`

Chains a callback to execute when the promise resolves.

```pleno
fetchData()
    .then(fn(data) {
        print("Got data: " + data);
        return processData(data);
    })
    .then(fn(result) {
        print("Processed: " + result);
    });
```

## Combinators

### `Promise.all(promises: Promise[]): Promise`

Waits for all promises to resolve. Returns array of results.

```pleno
let [a, b, c] = await Promise.all([
    fetchA(),
    fetchB(),
    fetchC()
]);
```

### `Promise.race(promises: Promise[]): Promise`

Returns when the first promise resolves.

```pleno
let fastest = await Promise.race([
    fetchFromServer1(),
    fetchFromServer2()
]);
```

## Spawning Tasks

### `spawn(fn: Function, ...args): Promise`

Spawns a concurrent task and returns a promise.

```pleno
let task = spawn(heavyComputation, data);
// Continue doing other work...
let result = await task;
```

---
