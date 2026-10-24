# std:concurrency

Concurrency primitives: Mutex and Channel. They coordinate tasks started with `spawn` or async functions.

## Import

```pleno
import { Mutex, Channel } from "std:concurrency";
```

---

## Mutex

A mutual exclusion lock for code that must not run in two tasks at once.

### `Mutex(): Mutex`

Creates a new unlocked mutex.

```pleno
import { Mutex } from "std:concurrency";

let mutex = Mutex();
```

### `lock(): Null | Promise`

Acquires the lock. Returns `null` right away if the mutex is unlocked, or a Promise that resolves once the lock is free. Always `await` the result.

### `unlock(): Null`

Releases the lock. If tasks are waiting, the first one gets the lock next. Unlocking an unlocked mutex does nothing.

```pleno
import { Mutex } from "std:concurrency";

let mutex = Mutex();
let mut counter = 0;

async fn add(n: Int) {
    await mutex.lock();
    counter = counter + n;   // critical section
    mutex.unlock();
}

await Promise.all([add(1), add(2)]);
print(counter); // 3
```

---

## Channel

Message-passing channel for communication between tasks.

### `Channel(capacity: Int = 0): Channel`

Creates a new channel. Capacity 0 (the default) means unbounded.

```pleno
import { Channel } from "std:concurrency";

let ch = Channel();          // unbounded
let buffered = Channel(10);  // holds at most 10 values
```

### `send(value: Any): Null | Promise`

Sends a value. Returns `null` if the value was delivered or buffered. If the buffer is full, returns a Promise that resolves once there is room; `await` it to wait. Sending on a closed channel throws an error.

### `recv(): Any | Promise`

Receives the next value. Returns it directly if one is buffered, otherwise a Promise that resolves with the next value sent. Use `await ch.recv()` in both cases. On a closed, empty channel it returns `Missing`.

```pleno
import { Channel } from "std:concurrency";

let ch = Channel();
ch.send("hello");
let msg = await ch.recv();
print(msg); // hello
```

### `close(): Null`

Closes the channel. Values already sent can still be received. Further sends throw an error.

```pleno
import { Channel } from "std:concurrency";

let ch = Channel();
ch.close();

try {
    ch.send(1);
} catch (e) {
    print(e.message); // send on closed channel
}
```

### Iteration

A `for` loop over a channel receives values until the channel is closed and empty:

```pleno
import { Channel } from "std:concurrency";

let ch = Channel();
ch.send("a");
ch.send("b");
ch.close();

for (message in ch) {
    print(message); // a, then b
}
```

---

## Example: Producer-Consumer

```pleno
import { Channel } from "std:concurrency";

let ch = Channel(2);

// Producer: waits whenever the buffer is full
spawn(async fn() {
    for (i in 0..5) {
        await ch.send(i);
    }
    ch.close();
});

// Consumer
for (item in ch) {
    print(`Received: ${item}`);
}
```

---
