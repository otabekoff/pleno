# Pleno Programming Language
## Complete Language Specification v1.0

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices

---

## 12. Memory Management & Performance

### 12.1 Ownership Hints (Optional)

```pleno
// Move semantics (transfer ownership)
fn takeOwnership(value: move String) {
    // value is moved, original is invalidated
}

// Borrow (read-only reference)
fn borrowValue(value: &String) {
    // value is borrowed, cannot be modified
}

// Mutable borrow
fn borrowMutable(value: &mut String) {
    // value can be modified
}
```

### 12.2 Lazy Evaluation

```pleno
// Lazy evaluation
let lazy lazyValue = expensiveComputation();
// Computed only when accessed
print(lazyValue);  // Computation happens here

// Lazy sequences
let lazySeq = (0..)
    .map(|x| => x * x)
    .filter(|x| => x % 2 == 0)
    .take(10);  // Only computes 10 items
```

### 12.3 Memory Pooling

```pleno
// Object pooling
@pooled(size: 100)
class GameObject {
    // Instances recycled from pool
}
```

---

## 13. Standard Library Highlights

### 13.1 String Operations

```pleno
let str = "Hello, World!";

str.length;                    // 13
str.toUpperCase();             // "HELLO, WORLD!"
str.toLowerCase();             // "hello, world!"
str.trim();                    // Remove whitespace
str.split(",");                // ["Hello", " World!"]
str.contains("World");         // true
str.startsWith("Hello");       // true
str.endsWith("!");             // true
str.replace("World", "Pleno"); // "Hello, Pleno!"
str.substring(0, 5);           // "Hello"
str.charAt(0);                 // "H"
str.indexOf("o");              // 4
str.repeat(3);                 // "Hello, World!Hello, World!Hello, World!"

// String interpolation
let name = "Alice";
let age = 30;
let message = `Hello, ${name}! You are ${age} years old.`;

// Multi-line strings
let multiline = `
    This is a
    multi-line
    string
`;
```

### 13.2 Math Operations

```pleno
Math.abs(-5);           // 5
Math.ceil(4.3);         // 5
Math.floor(4.7);        // 4
Math.round(4.5);        // 5
Math.max(1, 5, 3);      // 5
Math.min(1, 5, 3);      // 1
Math.pow(2, 3);         // 8
Math.sqrt(16);          // 4
Math.random();          // Random float between 0 and 1
Math.randomInt(1, 10);  // Random int between 1 and 10
Math.PI;                // 3.141592653589793
Math.E;                 // 2.718281828459045
```

### 13.3 File I/O

```pleno
import { File, FileMode } from "std:io";

// Read entire file
let content = File.read("data.txt");

// Read lines
let lines = File.readLines("data.txt");
for (line in lines) {
    print(line);
}

// Write to file
File.write("output.txt", "Hello, World!");

// Append to file
File.append("log.txt", "New log entry\n");

// File operations
File.exists("data.txt");       // Check if exists
File.delete("temp.txt");       // Delete file
File.copy("src.txt", "dst.txt"); // Copy file
File.move("old.txt", "new.txt"); // Move/rename file

// Working with file handles
let file = File.open("data.txt", FileMode.Read);
try {
    let content = file.read();
    print(content);
} finally {
    file.close();
}

// Using with auto-close
with (file = File.open("data.txt", FileMode.Read)) {
    let content = file.read();
    print(content);
}  // Automatically closed
```

### 13.4 Directory Operations

```pleno
import { Directory } from "std:io";

Directory.create("new_folder");
Directory.exists("folder");
Directory.delete("old_folder");
Directory.list(".");              // List files and folders
Directory.listFiles(".");         // List only files
Directory.listDirectories(".");   // List only directories

// Recursive operations
Directory.createRecursive("path/to/nested/folder");
Directory.deleteRecursive("folder_with_contents");
```

### 13.5 JSON Handling

```pleno
import { JSON } from "std:json";

// Parse JSON
let data = JSON.parse('{"name": "Alice", "age": 30}');
print(data.name);  // "Alice"

// Stringify to JSON
let obj = { name: "Bob", age: 25, active: true };
let jsonStr = JSON.stringify(obj);
// '{"name":"Bob","age":25,"active":true}'

// Pretty print
let prettyJson = JSON.stringify(obj, indent: 2);
```

### 13.6 HTTP Client

```pleno
import { Http } from "std:http";

// GET request
async fn fetchData() {
    let response = await Http.get("https://api.example.com/data");
    
    if (response.status == 200) {
        let data = JSON.parse(response.body);
        return data;
    } else {
        throw Error("Request failed: " + response.status);
    }
}

// POST request
async fn createUser(user: User) {
    let response = await Http.post(
        "https://api.example.com/users",
        body: JSON.stringify(user),
        headers: {
            "Content-Type": "application/json"
        }
    );
    
    return JSON.parse(response.body);
}

// Other methods
await Http.put(url, body: data);
await Http.delete(url);
await Http.patch(url, body: data);
```

### 13.7 Date & Time

```pleno
import { DateTime, Duration } from "std:time";

// Current date/time
let now = DateTime.now();
print(now.year);         // 2026
print(now.month);        // 1-12
print(now.day);          // 1-31
print(now.hour);         // 0-23
print(now.minute);       // 0-59
print(now.second);       // 0-59

// Create specific date
let date = DateTime(2026, 1, 22, 14, 30, 0);

// Formatting
now.format("YYYY-MM-DD HH:mm:ss");
now.format("DD/MM/YYYY");
now.toISO();  // ISO 8601 format

// Parsing
let parsed = DateTime.parse("2026-01-22", format: "YYYY-MM-DD");

// Duration
let duration = Duration.hours(2);
let future = now.add(duration);
let past = now.subtract(Duration.days(7));

// Comparison
if (date1 > date2) {
    print("date1 is later");
}
```

### 13.8 Regular Expressions

```pleno
import { Regex } from "std:regex";

let pattern = Regex(r"\d+");  // Raw string literal

// Test match
if (pattern.test("123")) {
    print("Matched!");
}

// Find matches
let matches = pattern.findAll("abc 123 def 456");
// ["123", "456"]

// Replace
let result = pattern.replace("abc 123 def", "XXX");
// "abc XXX def"

// Capture groups
let emailPattern = Regex(r"(\w+)@(\w+\.\w+)");
let match = emailPattern.match("user@example.com");
if (match) {
    print(match.group(1));  // "user"
    print(match.group(2));  // "example.com"
}
```

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