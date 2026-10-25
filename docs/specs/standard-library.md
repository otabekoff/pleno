# Standard Library Highlights

## Global Functions

These names are available in every program without an import:

| Name | What it does |
| --- | --- |
| `print(...values, sep = " ", end = "\n")` | Writes the values to standard output, separated by `sep` and followed by `end` |
| `input(prompt?)` | Prints the prompt, reads a line from standard input and returns it without the newline |
| `assert(condition, message?)` | Throws an `AssertionError` when the condition is false or `null` |
| `typeof(value)` | The type name: `"Int"`, `"String"`, `"Array"`, a class name, ... |
| `Int.parse(s)`, `Float.parse(s)` | Parse a number; throw for text that is not one |
| `map`, `filter`, `reduce` | Function forms of the array methods, for the pipeline operator `\|>` |
| `id(value)` | Returns its argument |
| `Set(array)`, `Error(message)`, `Promise(executor)`, `spawn(fn, ...args)` | See [Data structures](./data-structures.md), [Error handling](./error-handling.md) and [Async](./async-concurrency.md) |
| `Ok`, `Err`, `Some`, `None` | See [std:core](/stdlib/core) |

```pleno
print("A", "B", "C", sep = ", ");   // A, B, C
print("A", end = "!");
print("B");                          // A!B
print(Int.parse("42") + 1);          // 43
print(typeof(2.5));                  // Float
```

## String Operations

Positions count characters. With text that is not ASCII, `length`, `indexOf`, `substring` and `charAt` currently count bytes; see [Strings](./index.md#strings).

```pleno
let str = "Hello, World!";

print(str.length);                    // 13
print(str.toUpperCase());             // HELLO, WORLD!
print(str.toLowerCase());             // hello, world!
print("  padded  ".trim());           // padded
print(str.split(","));                // [Hello,  World!]
print(str.contains("World"));         // true
print(str.startsWith("Hello"));       // true
print(str.endsWith("!"));             // true
print(str.replace("World", "Pleno")); // Hello, Pleno!
print(str.substring(0, 5));           // Hello
print(str.charAt(0));                 // H
print(str.indexOf("o"));              // 4
print("ab".repeat(3));                // ababab

// String interpolation
let name = "Alice";
let age = 30;
let message = `Hello, ${name}! You are ${age} years old.`;
print(message);  // Hello, Alice! You are 30 years old.

// Multi-line strings
let multiline = `
    This is a
    multi-line
    string
`;
```

## Math Operations

```pleno
import { Math } from "std:math";

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

## File I/O

```pleno
import { File } from "std:io";

// Write a file, then read it back
File.write("data.txt", "line 1
line 2");
let content = File.read("data.txt");

// Read lines
let lines = File.readLines("data.txt");
for (line in lines) {
    print(line);  // line 1, then line 2
}

// Append to a file
File.append("log.txt", "New log entry
");

// File operations
print(File.exists("data.txt"));     // true
File.copy("data.txt", "copy.txt");  // Copy file
File.move("copy.txt", "new.txt");   // Move or rename file

// Working with file handles: mode "r" (read), "w" (write) or "a" (append)
let file = File.open("data.txt", "r");
try {
    print(file.read());
} finally {
    file.close();
}

// Delete files
File.delete("data.txt");
File.delete("log.txt");
File.delete("new.txt");
```

Pleno has no statement that closes a file automatically; close file handles in a `finally` block as above.

## Directory Operations

```pleno
import { Directory, File } from "std:io";

Directory.create("box");
print(Directory.exists("box"));       // true
File.write("box/a.txt", "a");
Directory.create("box/inner");

print(Directory.list("box"));             // [a.txt, inner]
print(Directory.listFiles("box"));        // [a.txt]
print(Directory.listDirectories("box"));  // [inner]
Directory.delete("box/inner");            // Delete an empty folder

// Recursive operations
Directory.createRecursive("path/to/nested/folder");
Directory.deleteRecursive("path");
Directory.deleteRecursive("box");
```

## JSON Handling

```pleno
import { JSON } from "std:json";

// Parse JSON
let data = JSON.parse(`{"name": "Alice", "age": 30}`);
print(data.name);  // Alice

// Stringify to JSON
let obj = { "name": "Bob", "age": 25, "active": true };
print(JSON.stringify(obj));
// {"name":"Bob","age":25,"active":true}

// Pretty print, indented by 2 spaces
print(JSON.stringifyPretty(obj, indent: 2));
```

## HTTP Client

Requests are blocking and need no `await`. `get`, `post`, `put`, `patch` and `delete` return the response body. `request` returns a map with `status`, `ok`, `headers` and `body`. See [std:http](/stdlib/http).

```pleno norun
import { HTTP } from "std:http";
import { JSON } from "std:json";

// GET request
fn fetchData() {
    let response = HTTP.request("GET", "https://api.example.com/data");

    if (response.status == 200) {
        return JSON.parse(response.body);
    } else {
        throw Error("Request failed: " + response.status.toString());
    }
}

// POST request with headers
fn createUser(user: Map) {
    let response = HTTP.request(
        "POST",
        "https://api.example.com/users",
        JSON.stringify(user),
        { "Content-Type": "application/json" }
    );
    return JSON.parse(response.body);
}

// Other methods
let url = "https://api.example.com/users/1";
HTTP.put(url, `{"name": "Ann"}`);
HTTP.patch(url, `{"name": "Ann"}`);
HTTP.delete(url);
```

## Date & Time

```pleno
import { DateTime, Duration } from "std:time";

// Current date/time
let now = DateTime.now();
print(now.getTimestamp());         // Unix timestamp

// Formatting (chrono-style format)
print(now.format("%Y-%m-%d %H:%M:%S"));

// Parsing
let parsed = DateTime.parse("2026-01-22", "%Y-%m-%d");
print(parsed.year);                // 2026

// Duration
let duration = Duration.hours(2);
let future = now.add(duration);
let past = now.subtract(Duration.days(7));

if (now.isAfter(past)) {
    print("now is later");
}
```

## Regular Expressions

```pleno
import { Regex } from "std:regex";

let pattern = Regex(r"\d+");  // Raw string literal

// Test match
if (pattern.test("123")) {
    print("Matched!");
}

// Find matches
let matches = pattern.findAll("abc 123 def 456");
print(matches.length);    // 2
print(matches[0].value);  // 123

// Replace
print(pattern.replace("abc 123 def", "XXX"));  // abc XXX def

// Capture groups
let emailPattern = Regex(r"(\w+)@(\w+\.\w+)");
let found = emailPattern.match("user@example.com");
if (found) {
    print(found.group(1));  // user
    print(found.group(2));  // example.com
}
```

## Low-level Networking

`std:net` is experimental and not covered by the [compatibility promise](./compatibility.md). `listen` and `connect` only check that an address can be bound or reached; the socket is closed right away. See [std:net](/stdlib/net).

```pleno
import { Net } from "std:net";

// TCP listener on any free port
let listener = Net.listen("127.0.0.1:0");

// TCP connection; throws if it fails
try {
    Net.connect("127.0.0.1:1");
} catch (e) {
    print("Cannot connect: " + e.message);
}
```
