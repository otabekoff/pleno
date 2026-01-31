# Pleno Programming Language
## Complete Language Specification v1.0

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices



---

## 14. Advanced Features

### 14.1 Reflection & Metaprogramming

```pleno
import { Reflect } from "std:reflect";

class User {
    public name: String;
    public age: Int;
}

let user = User();

// Get class info
let className = Reflect.getTypeName(user);  // "User"
let fields = Reflect.getFields(user);       // ["name", "age"]
let methods = Reflect.getMethods(user);

// Get/set field values dynamically
Reflect.setField(user, "name", "Alice");
let name = Reflect.getField(user, "name");

// Check type at runtime
if (Reflect.isInstanceOf(user, User)) {
    print("Is a User instance");
}

// Call method dynamically
Reflect.callMethod(user, "someMethod", [arg1, arg2]);
```

### 14.2 Macros (Compile-time Code Generation)

```pleno
// Define macro
macro repeat(count, code) {
    for (let i = 0; i < count; i++) {
        ${code}
    }
}

// Use macro
repeat!(3, {
    print("Hello");
});

// Expands to:
// print("Hello");
// print("Hello");
// print("Hello");
```

### 14.3 Operator Overloading

```pleno
class Vector {
    private x: Float;
    private y: Float;
    
    constructor(x: Float, y: Float) {
        this.x = x;
        this.y = y;
    }
    
    // Overload + operator
    operator +(other: Vector): Vector {
        return Vector(this.x + other.x, this.y + other.y);
    }
    
    // Overload * operator (scalar multiplication)
    operator *(scalar: Float): Vector {
        return Vector(this.x * scalar, this.y * scalar);
    }
    
    // Overload == operator
    operator ==(other: Vector): Bool {
        return this.x == other.x && this.y == other.y;
    }
    
    // Overload [] operator
    operator [](index: Int): Float {
        match (index) {
            case 0 => return this.x,
            case 1 => return this.y,
            case _ => throw Error("Index out of bounds")
        }
    }
}

let v1 = Vector(1.0, 2.0);
let v2 = Vector(3.0, 4.0);
let v3 = v1 + v2;          // Vector(4.0, 6.0)
let v4 = v1 * 2.0;         // Vector(2.0, 4.0)
print(v1 == v2);           // false
print(v1[0]);              // 1.0
```

### 14.4 Extension Methods

```pleno
// Extend existing types
extend String {
    fn reversed(): String {
        return this.split("").reverse().join("");
    }
    
    fn isPalindrome(): Bool {
        return this == this.reversed();
    }
}

// Usage
print("hello".reversed());      // "olleh"
print("racecar".isPalindrome()); // true

// Extend generic types
extend <T> Array<T> {
    fn second(): T? {
        if (this.length > 1) {
            return Some(this[1]);
        }
        return None;
    }
}

let numbers = [1, 2, 3];
print(numbers.second());  // Some(2)
```

### 14.5 Annotations & Metadata

```pleno
// Define custom annotation
@annotation
class Route {
    public path: String;
    public method: String;
    
    constructor(path: String, method: String = "GET") {
        this.path = path;
        this.method = method;
    }
}

// Use annotation
@Route("/users", "GET")
fn getUsers(): User[] {
    // implementation
}

@Route("/users", "POST")
fn createUser(user: User): User {
    // implementation
}

// Read annotations at runtime
let annotations = Reflect.getAnnotations(getUsers);
```

### 14.6 Enum Types

```pleno
// Simple enum
enum Color {
    Red,
    Green,
    Blue
}

let color = Color.Red;

// Enum with values
enum Status {
    Pending = 0,
    Active = 1,
    Completed = 2
}

// Enum with associated data
enum Result<T, E> {
    Ok(T),
    Err(E)
}

let success: Result<Int, String> = Result.Ok(42);
let failure: Result<Int, String> = Result.Err("Failed");

// Pattern matching on enums
match (success) {
    case Result.Ok(value) => print("Success: " + value),
    case Result.Err(error) => print("Error: " + error)
}

// Enum methods
enum Direction {
    North,
    South,
    East,
    West;
    
    fn opposite(): Direction {
        match (this) {
            case Direction.North => return Direction.South,
            case Direction.South => return Direction.North,
            case Direction.East => return Direction.West,
            case Direction.West => return Direction.East
        }
    }
}
```

### 14.7 Type Aliases

```pleno
// Simple type alias
type UserId = Int;
type UserName = String;

// Generic type alias
type Result<T> = Result<T, Error>;
type HashMap<K, V> = Map<K, V>;

// Complex type alias
type Handler = (Request, Response) => Void;
type Predicate<T> = (T) => Bool;

// Using type aliases
fn processUser(id: UserId, name: UserName): Void {
    // implementation
}

let handler: Handler = fn(req, res) => {
    res.send("Hello");
};
```

### 14.8 Union & Intersection Types

```pleno
// Union type
type StringOrNumber = String | Int | Float;

fn process(value: StringOrNumber) {
    if (value is String) {
        print(value.toUpperCase());
    } else {
        print(value * 2);
    }
}

// Intersection type (must satisfy all constraints)
interface Nameable {
    fn getName(): String;
}

interface Ageable {
    fn getAge(): Int;
}

type Person = Nameable & Ageable;

// Function accepting intersection type
fn displayPerson(p: Person) {
    print(p.getName() + " is " + p.getAge() + " years old");
}
```

---

## 15. Testing Framework

```pleno
import { test, assert, describe, beforeEach, afterEach } from "std:test";

// Test suite
describe("Calculator", fn() {
    let calc: Calculator;
    
    beforeEach(fn() {
        calc = Calculator();
    });
    
    afterEach(fn() {
        calc = null;
    });
    
    test("should add numbers correctly", fn() {
        let result = calc.add(2, 3);
        assert.equal(result, 5);
    });
    
    test("should handle negative numbers", fn() {
        let result = calc.add(-5, 3);
        assert.equal(result, -2);
    });
    
    test("should throw on division by zero", fn() {
        assert.throws(fn() => calc.divide(10, 0));
    });
});

// Assertions
assert.equal(actual, expected);
assert.notEqual(actual, expected);
assert.true(value);
assert.false(value);
assert.null(value);
assert.notNull(value);
assert.throws(fn);
assert.contains(collection, item);
assert.lengthOf(collection, length);
```

---

## 16. Package Manager (PLM - Pleno Module Manager)

```bash
# Initialize new project
plm init

# Install package
plm install http-client

# Install specific version
plm install json-parser@1.5.0

# Install dev dependency
plm install --dev test-framework

# Update packages
plm update

# Remove package
plm remove http-client

# Run script
plm run build
plm run test
```

```pleno
// package.pln
{
    "name": "my-app",
    "version": "1.0.0",
    "description": "My Pleno application",
    "main": "src/main.pln",
    "scripts": {
        "start": "pleno run src/main.pln",
        "build": "pleno build --output dist/",
        "test": "pleno test tests/"
    },
    "dependencies": {
        "http-client": "^2.1.0",
        "json-parser": "^1.5.0"
    },
    "devDependencies": {
        "test-framework": "^3.0.0"
    }
}
```

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