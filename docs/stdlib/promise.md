# std:promise

Promises and tasks for asynchronous code. `Promise` and `spawn` are also globals, so the import is optional.

## Import

```pleno
import { Promise, spawn } from "std:promise";
```

Calling an `async fn` returns a Promise. `await` waits for a Promise and returns its value, or throws its rejection reason.

## Creating Promises

### `Promise(executor: Function): Promise`

Creates a Promise. The executor is called right away with two functions: `resolve(value)` fulfills the promise and `reject(reason)` rejects it. If the executor throws, the promise is rejected.

```pleno
import { Promise } from "std:promise";

fn checkPositive(n: Int) {
    return Promise(fn(resolve, reject) {
        if (n > 0) {
            resolve(n * 2);
        } else {
            reject("n must be positive");
        }
    });
}

print(await checkPositive(5)); // 10

try {
    await checkPositive(0);
} catch (e) {
    print(`failed: ${e}`); // failed: n must be positive
}
```

### `Promise.resolve(value: Any): Promise`

Returns a promise already fulfilled with `value`.

### `Promise.reject(reason: Any): Promise`

Returns a promise already rejected with `reason`. Handle it with `await` inside `try`, or with `catch`, or the run fails with an unhandled rejection.

```pleno
import { Promise } from "std:promise";

print(await Promise.resolve(42)); // 42

try {
    await Promise.reject("boom");
} catch (e) {
    print(e); // boom
}
```

## Chaining

### `then(onFulfilled: Function, onRejected: Function?): Promise`

Calls `onFulfilled` with the value when the promise resolves, or `onRejected` with the reason when it rejects. Returns a new promise for the callback's result.

### `catch(onRejected: Function): Promise`

Calls `onRejected` with the reason if the promise rejects.

### `finally(onFinally: Function): Promise`

Calls `onFinally` with no arguments once the promise settles either way.

```pleno
import { Promise } from "std:promise";

async fn fetchCount(): Int {
    return 2;
}

fetchCount()
    .then(fn(n) => n + 1)
    .then(fn(n) {
        print(`count: ${n}`); // count: 3
    });

Promise.reject("network down")
    .catch(fn(e) {
        print(`error: ${e}`); // error: network down
    })
    .finally(fn() {
        print("done");
    });
```

## Combinators

### `Promise.all(promises: Promise[]): Promise`

Waits for all promises to resolve and returns an array of their results, in the same order. Rejects as soon as one of them rejects.

```pleno
import { Promise } from "std:promise";

async fn double(x: Int): Int {
    return x * 2;
}

let results = await Promise.all([double(1), double(2), double(3)]);
print(results); // [2, 4, 6]
```

### `Promise.race(promises: Promise[]): Promise`

Settles with the first promise in the array that settles. The array must not be empty.

```pleno
import { Promise } from "std:promise";

async fn answer(x: Int): Int {
    return x;
}

let first = await Promise.race([answer(1), answer(2)]);
print(first); // 1
```

## Spawning Tasks

### `spawn(fn: Function, ...args): Promise`

Runs `fn(...args)` as a separate task and returns a promise for its result.

```pleno
import { spawn } from "std:promise";

fn sum(a: Int, b: Int): Int {
    return a + b;
}

let task = spawn(sum, 2, 3);
// Do other work here...
print(await task); // 5
```

---
