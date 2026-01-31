# Pleno Programming Language
## Complete Language Specification v1.0

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices

---



---

## 17. Tooling & CLI

```bash
# Run Pleno file
pleno run script.pln

# Run with arguments
pleno run script.pln arg1 arg2

# Compile to bytecode
pleno compile main.pln --output main.plc

# Type check without running
pleno check main.pln

# Format code
pleno format src/

# Lint code
pleno lint src/

# Start REPL
pleno

# Watch mode (auto-reload)
pleno watch main.pln

# Build optimized binary
pleno build --release --output app

# Generate documentation
pleno doc --output docs/
```

---

## 18. Example Programs

### 18.1 Hello World

```pleno
fn main() {
    print("Hello, World!");
}

main();
```

### 18.2 Web Server

```pleno
import { Server, Router } from "std:http";

let router = Router();

router.get("/", fn(req, res) {
    res.send("Hello, Pleno!");
});

router.get("/users/:id", fn(req, res) {
    let userId = req.params.id;
    res.json({ id: userId, name: "User " + userId });
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

### 18.3 Todo CLI Application

```pleno
import { File } from "std:io";
import { JSON } from "std:json";

class TodoManager {
    private todos: Array<Todo> = [];
    private file: String = "todos.json";
    
    constructor() {
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
            print(`${index + 1}. ${status} ${todo.title}`);
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
    
    constructor(title: String) {
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

---

## 19. Performance Features

### 19.1 JIT Compilation
- Hot path optimization
- Automatic inlining of small functions
- Dead code elimination

### 19.2 Memory Management
- Generational garbage collection
- Reference counting for immediate cleanup
- Optional manual memory hints
- Object pooling support

### 19.3 Optimization Hints

```pleno
// Inline hint
@inline
fn fastOperation(x: Int): Int {
    return x * 2;
}

// No optimization (for debugging)
@no_optimize
fn debugFunction() {
    // Keep unoptimized for debugging
}

// Hot path hint
@hot
fn criticalPath() {
    // Prioritize optimization
}
```

---

## 20. Security Features

### 20.1 Safe Defaults
- No implicit type coercion
- Strict null checking
- Bounds checking on arrays
- Integer overflow detection

### 20.2 Sandboxing

```pleno
import { Sandbox } from "std:security";

// Run untrusted code in sandbox
let sandbox = Sandbox({
    allowFileAccess: false,
    allowNetworkAccess: false,
    memoryLimit: 10 * 1024 * 1024  // 10 MB
});

let result = sandbox.run(fn() {
    // Untrusted code here
    return calculate(data);
});
```

---

## Conclusion

Pleno is designed to be:
- **Easy to learn**: Familiar syntax, clear semantics
- **Safe by default**: Strong typing, real encapsulation, memory safety
- **Powerful**: OOP, functional, and procedural paradigms
- **Modern**: Async/await, pattern matching, generics
- **Practical**: Great standard library, package manager, tooling

This specification provides a complete foundation for a modern programming language suitable for 2026 and beyond.