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