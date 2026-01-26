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

### 13.6 HTTP
```pleno
import http from "std:http";
import { JSON } from "std:json";

// --- HTTP Client ---

// GET request
let responseBody = http.get("https://api.example.com/data");
let data = JSON.parse(responseBody);

// POST request
let newUser = { name: "Alice" };
let resp = http.post("https://api.example.com/users", JSON.stringify(newUser));

// Other methods
http.put("url", "body");
http.delete("url");

// --- HTTP Server ---

let router = http.Router();

router.get("/", fn(req, res) {
    println("Received request");
});

router.post("/api/data", fn(req, res) {
    println("Received data");
});

let server = http.Server(router);
// server.listen(8080); // Blocking call
```

### 13.7 Date & Time
```pleno
import time from "std:time";

// Current Unix timestamp (float seconds)
let now = time.now();
print(now);

// Formatting
let dateStr = time.format(now, "%Y-%m-%d %H:%M:%S"); // "2026-01-22 14:30:00"

// Parsing
let timestamp = time.parse("2026-01-22", "%Y-%m-%d");

// Struct access
let parts = time.parts(now);
print(parts.year);
print(parts.month);
print(parts.second);
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

### 13.9 Path Operations

```pleno
import path from "std:path";

// Join path components
let fullPath = path.join("folder", "subfolder", "file.txt");

// Get directory name
path.dirname("/home/user/file.txt");

// Get file name
path.basename("/home/user/file.txt");

// Get extension
path.extname("document.pdf");

// Check existence
path.exists("config.json");
path.isFile("config.json");
path.isDir("src");

// Resolve to absolute path
path.resolve("./relative/path");

// Path separator
path.separator;
```

### 13.10 Process Management

```pleno
import process from "std:process";

// Get current working directory
let cwd = process.cwd();

// Get command line arguments
let args = process.args();

// Get environment variable
let home = process.env("HOME");

// Get process ID
let pid = process.pid();

// Exit with status code
process.exit(0);
```

