# Examples

Practical examples demonstrating common Pleno patterns.

## Basic

### Hello World

```pleno
fn main() {
    print("Hello, World!");
}

main();
```

### Variables & Types

```pleno
// Immutable by default
let name = "Alice";
let age = 30;
let pi = 3.14159;

// Mutable variable
let mut counter = 0;
counter = counter + 1;

// Constants
const MAX_SIZE = 100;

// Type annotations
let score: Int = 95;
let active: Bool = true;
let data: String? = null;  // Optional
```

---

## Web Server

### REST API

A small REST API using `std:http`. The server keeps running until you stop it.

```pleno norun
import { HTTP } from "std:http";
import { JSON } from "std:json";

let router = HTTP.Router();

// GET /
router.get("/", fn(req, res) {
    res.send("Welcome to Pleno API!");
});

// GET /users/:id
router.get("/users/:id", fn(req, res) {
    let userId = req.params.id;
    res.json({ "id": userId, "name": "User " + userId });
});

// POST /users
router.post("/users", fn(req, res) {
    let user = JSON.parse(req.body);
    res.status(201).json(user);
});

let server = HTTP.Server(router);
server.listen(8080, fn() {
    print("Server running on http://localhost:8080");
});
```

---

## CLI Applications

### Todo Manager

Command-line task manager that saves its tasks to `todos.json`. Tasks are stored as maps, because `JSON.stringify` writes class instances as `null`.

```pleno
import { File } from "std:io";
import { JSON } from "std:json";
import { Process } from "std:process";

class TodoManager {
    private todos: Array;
    private file: String = "todos.json";

    constructor() {
        this.todos = [];
        this.load();
    }

    fn add(title: String) {
        this.todos.push({ "title": title, "completed": false });
        this.save();
        print("Added: " + title);
    }

    fn list() {
        if (this.todos.length == 0) {
            print("No todos yet!");
            return;
        }
        for (i in 0..this.todos.length) {
            let todo = this.todos[i];
            let status = todo.completed ? "[x]" : "[ ]";
            print((i + 1).toString() + ". " + status + " " + todo.title);
        }
    }

    fn complete(index: Int) {
        if (index >= 0 && index < this.todos.length) {
            this.todos[index].completed = true;
            this.save();
            print("Completed: " + this.todos[index].title);
        }
    }

    private fn save() {
        File.write(this.file, JSON.stringify(this.todos));
    }

    private fn load() {
        if (File.exists(this.file)) {
            this.todos = JSON.parse(File.read(this.file));
        }
    }
}

fn main() {
    // args is [pleno, run, todo.pln, command, ...]
    let args = Process.args();
    let manager = TodoManager();

    if (args.length <= 3) {
        print("Usage: pleno run todo.pln [add|list|complete] ...");
        return;
    }

    match (args[3]) {
        case "add" => manager.add(args[4]),
        case "list" => manager.list(),
        case "complete" => manager.complete(Int.parse(args[4]) - 1),
        case _ => print("Unknown command")
    }
}

main();
```

Run it with a command:

```sh
pleno run todo.pln add "Buy milk"
pleno run todo.pln list
pleno run todo.pln complete 1
```

---

## Async & Concurrency

### Basic Async/Await

Calling an `async fn` returns a Promise. `await` waits for it. Top-level code can use `await` directly.

```pleno
import { sleep } from "std:time";

// Stands in for a slow operation such as a network request.
async fn fetchData(id: Int): String {
    await sleep(50);
    return "item " + id.toString();
}

async fn main() {
    let data = await fetchData(1);
    print("Received: " + data);  // Received: item 1
}

await main();
```

### Promise.all

Start several tasks, then wait for all of them. The results keep the order of the input.

```pleno
import { sleep } from "std:time";

// Stands in for a slow request.
async fn fetchUser(id: Int): Map {
    await sleep(50);
    return { "id": id, "name": "User " + id.toString() };
}

async fn fetchMultiple() {
    let mut promises = [];
    for (id in [1, 2, 3]) {
        promises.push(fetchUser(id));
    }
    let users = await Promise.all(promises);

    for (user in users) {
        print(user.name);  // User 1, User 2, User 3
    }
}

await fetchMultiple();
```

### Channel Communication

`spawn` is a global. It runs a function as a separate task. A `for` loop over a channel receives values until the channel is closed.

```pleno
import { Channel } from "std:concurrency";

async fn producer(ch: Channel) {
    for (i in 0..10) {
        await ch.send(i * 2);
    }
    ch.close();
}

fn consumer(ch: Channel) {
    for (value in ch) {
        print("Received: " + value.toString());  // 0, 2, 4, ... 18
    }
}

let ch = Channel(5);  // Buffered channel
spawn(producer, ch);
consumer(ch);
```

### Mutex for Shared State

Each task reads the counter, waits, then writes it back. The mutex keeps the tasks from overwriting each other's updates.

```pleno
import { Mutex } from "std:concurrency";
import { sleep } from "std:time";

let mutex = Mutex();
let mut counter = 0;

async fn increment(n: Int) {
    for (i in 0..n) {
        await mutex.lock();
        let current = counter;
        await sleep(0);
        counter = current + 1;
        mutex.unlock();
    }
}

await Promise.all([
    spawn(increment, 100),
    spawn(increment, 100),
    spawn(increment, 100)
]);
print("Final count: " + counter.toString());  // Final count: 300
```

---

## Data Processing

### JSON Handling

```pleno
import { JSON } from "std:json";

let data = JSON.parse(`{
    "name": "Alice",
    "age": 30,
    "skills": ["Pleno", "Rust", "TypeScript"]
}`);

print(data.name);           // Alice
print(data.skills[0]);      // Pleno

let output = JSON.stringify({ "status": "ok", "count": 42 });
print(output);
```

### CSV Processing

```pleno
import { CSV } from "std:csv";
import { File } from "std:io";

File.write("data.csv", "name,language\nAlice,Pleno\nBob,Rust\n");

let content = File.read("data.csv");
let rows = CSV.parse(content);  // the header row is skipped

for (row in rows) {
    print(row[0] + ": " + row[1]);  // Alice: Pleno, then Bob: Rust
}

File.delete("data.csv");
```

### Regex Matching

```pleno
import { Regex } from "std:regex";

let email = Regex(r"(\w+)@(\w+\.\w+)");
let found = email.match("user@example.com");

if (found != null) {
    print("User: " + found.group(1));     // user
    print("Domain: " + found.group(2));   // example.com
}

let numbers = Regex(r"\d+");
let all = numbers.findAll("abc 123 def 456");
print(all.map(fn(m) => m.value));     // [123, 456]
```

---

## Testing

### Unit Tests

```pleno
import { describe, test, assert } from "std:test";

describe("Calculator", fn() {
    test("addition", fn() {
        assert.equal(1 + 1, 2);
    });

    test("subtraction", fn() {
        assert.equal(5 - 3, 2);
    });

    test("division by zero throws", fn() {
        assert.throws(fn() {
            let x = 1 / 0;
        });
    });
});
```

---

## Cryptography

### Hashing & JWT

```pleno
import { Crypto } from "std:crypto";

// Hash a password
let hash = Crypto.hash("sha256", "password123");
print(hash);

// Create JWT
let token = Crypto.jwtSign({
    "sub": "user123",
    "exp": 4102444800
}, "my-secret-key");

// Verify JWT
let payload = Crypto.jwtVerify(token, "my-secret-key");
print(payload.sub);  // user123
```

---

## More Resources

- [Standard Library Reference](/stdlib/)
- [Language Specification](/specs/)
- [Contributing Guidelines](/guide/contributing)
