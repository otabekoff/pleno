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

### 13.9 Path Operations

```pleno
import Path from "std:path";

// Join path components
let fullPath = Path.join("folder", "subfolder", "file.txt");
// "folder/subfolder/file.txt" (Unix) or "folder\subfolder\file.txt" (Windows)

// Get directory name
Path.dirname("/home/user/file.txt");  // "/home/user"

// Get file name
Path.basename("/home/user/file.txt");  // "file.txt"

// Get extension
Path.extname("document.pdf");  // ".pdf"

// Check existence
Path.exists("config.json");     // true/false
Path.isFile("config.json");     // true if file
Path.isDir("src");              // true if directory

// Resolve to absolute path
Path.resolve("./relative/path");  // "/absolute/path/to/relative/path"

// Path separator (platform-specific)
Path.separator;  // "/" on Unix, "\" on Windows
```

### 13.10 Process Management

```pleno
import Process from "std:process";

// Get current working directory
let cwd = Process.cwd();
print(cwd);  // "/home/user/project"

// Get command line arguments
let args = Process.args();
// ["pleno", "script.pln", "--verbose"]

// Get environment variable
let home = Process.env("HOME");
let path = Process.env("PATH");

// Get process ID
let pid = Process.pid();
print(pid);  // 12345

// Exit with status code
Process.exit(0);  // Success
Process.exit(1);  // Error
```

