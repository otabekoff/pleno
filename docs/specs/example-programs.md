# Example Programs

## Hello World

```pleno
fn main() {
    print("Hello, World!");
}

main();
```

## Web Server

The server keeps running until you stop it.

```pleno norun
import { Server, Router } from "std:http";
import { JSON } from "std:json";

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

## Todo CLI Application

```pleno
import { File } from "std:io";
import { JSON } from "std:json";
import { Process } from "std:process";

// Todos are maps, because JSON.stringify writes class instances as null.
class TodoManager {
    private todos: Array = [];
    private file: String = "todos.json";

    constructor() {
        this.load();
    }

    fn add(title: String) {
        this.todos.push({ "title": title, "completed": false });
        this.save();
        print("Added: " + title);
    }

    fn list() {
        if (this.todos.isEmpty()) {
            print("No todos yet!");
            return;
        }

        for (index, todo in this.todos) {
            let status = todo.completed ? "[x]" : "[ ]";
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

fn main(args: String[]) {
    let manager = TodoManager();

    if (args.length == 0) {
        print("Usage: pleno run todo.pln [add|list|complete|remove] ...");
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

// Process.args() is [pleno, run, todo.pln, ...]; the command starts at index 3.
main(Process.args()[3..]);
```

Run it with a command:

```sh
pleno run todo.pln add Buy some milk
pleno run todo.pln list
pleno run todo.pln complete 1
pleno run todo.pln remove 1
```
