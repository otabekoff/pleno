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

A simple REST API using `std:http`.

```pleno
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

Command-line task manager with file persistence.

```pleno
import { File } from "std:io";
import { JSON } from "std:json";
import { Process } from "std:process";

class Todo {
    public title: String;
    public completed: Bool = false;

    constructor(title: String) {
        this.title = title;
    }
}

class TodoManager {
    private todos: Array;
    private file: String = "todos.json";

    constructor() {
        this.todos = [];
        this.load();
    }

    fn add(title: String) {
        this.todos.push(Todo(title));
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
            let status = todo.completed ? "[✓]" : "[ ]";
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
    let args = Process.args();
    let manager = TodoManager();

    if (args.length <= 1) {
        print("Usage: todo [add|list|complete] ...");
        return;
    }

    match (args[1]) {
        case "add" => manager.add(args[2]),
        case "list" => manager.list(),
        case "complete" => manager.complete(Int.parse(args[2]) - 1),
        case _ => print("Unknown command")
    }
}

main();
```

---

## Async & Concurrency

### Basic Async/Await

```pleno
import { HTTP } from "std:http";

async fn fetchData(url: String): String {
    let response = await HTTP.get(url);
    return response.body;
}

async fn main() {
    let data = await fetchData("https://api.example.com/data");
    print("Received: " + data);
}

main();
```

### Promise.all

```pleno
async fn fetchMultiple() {
    let urls = [
        "https://api.example.com/user",
        "https://api.example.com/posts",
        "https://api.example.com/comments"
    ];

    let promises = urls.map(fn(url) => HTTP.get(url));
    let responses = await Promise.all(promises);

    for (response in responses) {
        print(response.body);
    }
}
```

### Channel Communication

```pleno
import { Channel, spawn } from "std:concurrency";

fn producer(ch: Channel) {
    for (i in 0..10) {
        ch.send(i * 2);
    }
    ch.close();
}

fn consumer(ch: Channel) {
    for (value in ch) {
        print("Received: " + value.toString());
    }
}

let ch = Channel(5);  // Buffered channel
spawn(fn() => producer(ch));
consumer(ch);
```

### Mutex for Shared State

```pleno
import { Mutex, spawn, sleep } from "std:concurrency";

let mutex = Mutex();
let counter = 0;

async fn increment(n: Int) {
    for (i in 0..n) {
        await mutex.lock();
        counter = counter + 1;
        mutex.unlock();
    }
}

async fn main() {
    spawn(fn() => increment(100));
    spawn(fn() => increment(100));
    spawn(fn() => increment(100));

    await sleep(500);
    print("Final count: " + counter.toString());  // 300
}
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

let content = File.read("data.csv");
let rows = CSV.parse(content);

for (row in rows) {
    print(row[0] + ": " + row[1]);
}
```

### Regex Matching

```pleno
import { Regex } from "std:regex";

let email = Regex(r"(\w+)@(\w+\.\w+)");
let match = email.match("user@example.com");

if (match != null) {
    print("User: " + match.group(1));     // user
    print("Domain: " + match.group(2));   // example.com
}

let numbers = Regex(r"\d+");
let all = numbers.findAll("abc 123 def 456");
// ["123", "456"]
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
    "exp": 1735689600
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
