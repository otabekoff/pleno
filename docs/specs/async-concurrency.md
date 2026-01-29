# Async & Concurrency

### 9.1 Async/Await

```pleno
// Async function
async fn fetchData(url: String): Promise<String> {
    let response = await httpGet(url);
    return response.body;
}

// Using async function
async fn main() {
    try {
        let data = await fetchData("https://api.example.com");
        print(data);
    } catch (e: Error) {
        print("Error: " + e.message);
    }
}
```

### 9.2 Promises

```pleno
// Creating a promise
let promise = Promise<Int>(|resolve, reject| => {
    // Async work
    if (success) {
        resolve(42);
    } else {
        reject(Error("Failed"));
    }
});

// Promise chaining
promise
    .then(|value| => value * 2)
    .then(|value| => print(value))
    .catch(|error| => print(error));

// Promise.all
let results = await Promise.all([
    fetchData("url1"),
    fetchData("url2"),
    fetchData("url3")
]);

// Promise.race
let fastest = await Promise.race([
    fetchData("url1"),
    fetchData("url2")
]);
```

### 9.3 Parallel Execution

```pleno
// Spawn concurrent task
let task1 = spawn(fn() => heavyComputation1());
let task2 = spawn(fn() => heavyComputation2());

// Wait for results
let result1 = await task1;
let result2 = await task2;
```

### 9.4 Channels (Message Passing)

```pleno
// Create channel
let channel = Channel<Int>();

// Send to channel
spawn(fn() => {
    for (i in 0..10) {
        channel.send(i);
    }
    channel.close();
});

// Receive from channel
for (value in channel) {
    print(value);
}
```