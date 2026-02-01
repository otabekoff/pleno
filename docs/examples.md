# Examples

Specification implementations demonstrating common Pleno patterns.

## Basic

### Hello World
[`examples/hello_world.pln`](https://github.com/otabekoff/pleno/blob/main/examples/hello_world.pln)

```pleno
fn main() {
    print("Hello, World!");
}

main();
```

## Web Server

### HTTP Server
[`examples/http_server.pln`](https://github.com/otabekoff/pleno/blob/main/examples/http_server.pln)

A simple REST API using `std:http`.

```pleno
import { Server, Router } from "std:http";

let router = Router();

router.get("/", fn(req, res) {
    res.send("Hello, Pleno!");
});

router.get("/users/:id", fn(req, res) {
    let userId = req.params.id;
    res.json({ "id": userId, "name": "User " + userId });
});

router.post("/users", fn(req, res) {
    let user = JSON.parse(req.body);
    // Save user to database
    res.status(201).json(user);
});

let server = Server(router);
server.listen(8080, fn() {
    print("Server running on port 8080");
});
```

## CLI Applications

### Todo Manager
[`examples/todo_app.pln`](https://github.com/otabekoff/pleno/blob/main/examples/todo_app.pln)

A complete command-line interface for managing tasks, demonstrating file I/O, JSON handling, and classes.

```pleno
import { File } from "std:io";
import { JSON } from "std:json";

class TodoManager {
    private todos;
    private file;
    
    fn constructor() {
        this.todos = [];
        this.file = "todos.json";
        this.load();
    }
    
    fn add(title: String) {
        let todo = Todo(title);
        this.todos.push(todo);
        this.save();
        print("Added: " + title);
    }
    
    fn list() {
        if (this.todos.isEmpty()) {
            print("No todos yet!");
            return;
        }
        
        for (index, todo in this.todos) {
            let status = todo.completed ? "[✓]" : "[ ]";
            print((index + 1) + ". " + status + " " + todo.title);
        }
    }
    
    fn complete(index: Int) {
        if (index >= 0 && index < this.todos.length) {
            this.todos[index].completed = true;
            this.save();
            print("Completed: " + this.todos[index].title);
        }
    }
    
    fn remove(index: Int) {
        if (index >= 0 && index < this.todos.length) {
            let removed = this.todos.remove(index);
            this.save();
            print("Removed: " + removed.title);
        }
    }
    
    private fn save() {
        File.write(this.file, JSON.stringify(this.todos));
    }
    
    private fn load() {
        if (File.exists(this.file)) {
            let content = File.read(this.file);
            this.todos = JSON.parse(content);
        }
    }
}

class Todo {
    public title: String;
    public completed: Bool = false;
    
    fn constructor(title: String) {
        this.title = title;
    }
}

fn main(args: String[]) {
    let manager = TodoManager();
    
    if (args.length == 0) {
        print("Usage: todo [add|list|complete|remove] ...");
        return;
    }
    
    match (args[0]) {
        case "add" => {
            if (args.length > 1) {
                manager.add(args[1..].join(" "));
            }
        },
        case "list" => manager.list(),
        case "complete" => {
             if (args.length > 1) {
                manager.complete(Int.parse(args[1]) - 1);
            }
        },
        case "remove" => {
            if (args.length > 1) {
                manager.remove(Int.parse(args[1]) - 1);
            }
        },
        case _ => print("Unknown command")
    }
}
```

## More Resources

- [Standard Library Specification](../stdlib/)
- [Contributing Guidelines](../guide/contributing.md)

## Async & Concurrency

### Basic Async/Await

```pleno
async fn fetchData(url: String) -> String {
    let response = await fetch(url);
    return await response.text();
}

async fn main() {
    let data = await fetchData("https://api.example.com/data");
    print("Received: " + data);
}

main();
```

### Promise.all Pattern

```pleno
async fn fetchMultiple() {
    let urls = [
        "https://api.example.com/user",
        "https://api.example.com/posts",
        "https://api.example.com/comments"
    ];
    
    // Execute all requests concurrently
    let promises = urls.map(|url| => fetch(url));
    let responses = await Promise.all(promises);
    
    for (response in responses) {
        print(await response.text());
    }
}
```

### Channel-based Communication

```pleno
import { Channel } from "std:concurrency";

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

let ch = Channel(5);  // Buffered channel with capacity 5
spawn producer(ch);
consumer(ch);
```

### Mutex for Shared State

```pleno
import { Mutex, spawn } from "std:concurrency";

let counter = Mutex(0);

async fn increment(n: Int) {
    for (i in 0..n) {
        let value = await counter.lock();
        counter.set(value + 1);
    }
}

async fn main() {
    let tasks = [
        spawn increment(100),
        spawn increment(100),
        spawn increment(100)
    ];
    
    await Promise.all(tasks);
    let final = await counter.lock();
    print("Final count: " + final.toString());  // Should be 300
}
```

### Promise.race for Timeouts

```pleno
async fn fetchWithTimeout(url: String, timeoutMs: Int) {
    let timeout = new Promise(fn(resolve, reject) {
        setTimeout(fn() { reject("Request timed out"); }, timeoutMs);
    });
    
    let request = fetch(url);
    
    return await Promise.race([request, timeout]);
}

async fn main() {
    try {
        let response = await fetchWithTimeout("https://api.slow.com", 5000);
        print("Success: " + await response.text());
    } catch (e) {
        print("Error: " + e);
    }
}
```
