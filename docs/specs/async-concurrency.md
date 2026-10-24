# Async & Concurrency

## Async/Await

Calling an `async fn` returns a Promise. `await` waits for the Promise and returns its value, or throws its rejection reason. `await` also works in top-level code.

```pleno
import { sleep } from "std:time";

// Stands in for a slow operation such as a network request
async fn fetchData(name: String): String {
    await sleep(10);
    if (name == "") {
        throw Error("name is empty");
    }
    return "data for " + name;
}

async fn main() {
    try {
        print(await fetchData("users"));  // data for users
        await fetchData("");
    } catch (e) {
        print("Error: " + e.message);     // Error: name is empty
    }
}

await main();
```

## Promises

`Promise(executor)` creates a promise. `Promise` is a global. See [std:promise](/stdlib/promise) for the full API.

```pleno
import { sleep } from "std:time";

async fn fetchData(name: String): String {
    await sleep(10);
    return "data for " + name;
}

// Creating a promise
let success = true;
let promise = Promise(fn(resolve, reject) {
    if (success) {
        resolve(42);
    } else {
        reject("Failed");
    }
});

// Promise chaining
await promise
    .then(fn(value) => value * 2)
    .then(fn(value) => print(value))   // 84
    .catch(fn(error) => print(error));

// Promise.all: all results, in order
let results = await Promise.all([
    fetchData("a"),
    fetchData("b"),
    fetchData("c")
]);
print(results);  // [data for a, data for b, data for c]

// Promise.race: the first to settle
let fastest = await Promise.race([
    fetchData("x"),
    fetchData("y")
]);
print(fastest);  // data for x
```

## Parallel Execution

`spawn(fn, ...args)` is a global. It runs `fn(...args)` as a separate task and returns a promise for the result. Tasks take turns on one thread; they do not run on several CPU cores.

```pleno
fn heavyComputation(n: Int): Int {
    let mut sum = 0;
    for (i in 0..n) {
        sum = sum + i;
    }
    return sum;
}

// Spawn concurrent tasks
let task1 = spawn(heavyComputation, 1000);
let task2 = spawn(heavyComputation, 2000);

// Wait for results
let result1 = await task1;
let result2 = await task2;
print(result1, result2);  // 499500 1999000
```

## Channels (Message Passing)

```pleno
import { Channel } from "std:concurrency";

// Create a channel that buffers up to 10 values
let channel = Channel(10);

// Send from a separate task
spawn(async fn() {
    for (i in 0..10) {
        await channel.send(i);
    }
    channel.close();
});

// Receive until the channel is closed
for (value in channel) {
    print(value);  // 0 to 9
}
```
