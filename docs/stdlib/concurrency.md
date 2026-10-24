# std:concurrency

Concurrency primitives: Mutex and Channel.

## Import

```pleno
import { Mutex, Channel } from "std:concurrency";
```

---

## Mutex

A mutual exclusion lock for synchronizing access to shared resources.

### `new(): Mutex`

Creates a new unlocked mutex.

```pleno
let mutex = Mutex();
```

### `lock(): Void | Promise`

Acquires the lock. Returns immediately if unlocked, or a Promise if blocked.

```pleno
await mutex.lock();
// Critical section
mutex.unlock();
```

### `unlock(): Void`

Releases the lock, allowing the next waiter (if any) to acquire it.

```pleno
mutex.unlock();
```

---

## Channel

Message-passing channel for inter-task communication.

### `new(capacity: Int = 0): Channel`

Creates a new channel. Capacity 0 means unbounded.

```pleno
let ch = Channel();          // Unbounded
let buffered = Channel(10);  // Buffered with capacity 10
```

### `send(value: Any): Void | Promise`

Sends a value to the channel. Blocks if buffer is full.

```pleno
ch.send("message");
```

### `recv(): Any | Promise`

Receives a value from the channel. Blocks if buffer is empty.

```pleno
let msg = await ch.recv();
```

### `close(): Void`

Closes the channel. Further sends will fail.

```pleno
ch.close();
```

### Iteration

Channels can be iterated with `for`:

```pleno
for (message in channel) {
    print(message);
}
```

---

## Example: Producer-Consumer

```pleno
let ch = Channel(5);

// Producer
spawn(fn() {
    for (i in 0..10) {
        ch.send(i);
    }
    ch.close();
});

// Consumer
for (item in ch) {
    print("Received: " + item.toString());
}
```

---
