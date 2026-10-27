# Error Handling

Pleno has two ways to report failure: **throwing** an error, which unwinds until a `catch` handles it, and **returning** a `Result` or `Option`, which the caller has to look at.

## Try and catch

`throw` accepts an `Error` (or a class that extends it). `catch (e: Type)` handles only that type; a plain `catch (e)` handles anything. `finally` always runs.

```pleno
class NetworkError extends Error {}
class ValidationError extends Error {}

fn load(kind: String) {
    if (kind == "network") {
        throw NetworkError("connection refused");
    }
    if (kind == "input") {
        throw ValidationError("name is required");
    }
    throw Error("something else went wrong");
}

for (kind in ["network", "input", "other"]) {
    try {
        load(kind);
    } catch (e: NetworkError) {
        print("Network error: " + e.message);
    } catch (e: ValidationError) {
        print("Validation error: " + e.message);
    } catch (e: Error) {
        print("General error: " + e.message);
    } finally {
        print("attempt finished");
    }
}
```

An error that nothing catches stops the program with exit status 1. It prints the message, the line that threw, and a backtrace of the calls that led there. A backtrace deeper than 60 calls shows the innermost 50 and the outermost 10, with a `... N more frames` line between them.

`return`, `break` and `continue` inside `try` or `catch` run the `finally` block before they leave it.

## Closing resources with `with`

`with` opens one or more resources, runs a block, and then calls `close()` on each resource, however the block ends: at its closing brace, through `return`, `break` or `continue`, or with an error.

```pleno
import { File } from "std:io";

with (out = File.open("notes.txt", "w")) {
    out.write("first line\n");
}   // out.close() runs here

with (input = File.open("notes.txt")) {
    print(input.read());    // first line
}
File.delete("notes.txt");
```

The rules:

- **A resource is any value with a `close()` method**: an instance of a class that defines `close`, a map with a `close` function, or a built-in handle. In the standard library these are `File` handles (`std:io`), `Database` connections (`std:db`) and `Channel`s (`std:concurrency`). `with` checks for `close` before the block runs; a value without it, `null` included, throws a `TypeError` such as `'with' needs a value with a close() method, got Int`.
- **Several resources** are separated by commas. They open from left to right and close in reverse order, so a resource can use the ones before it: `with (db = Database.connect(url), tx = Transaction(db)) { ... }` closes `tx` and then `db`. If opening a resource throws, the ones already open are closed and the block does not run.
- **The names** are visible in the later resources and in the block, and not after the statement. They cannot be reassigned, so the value that is closed is always the one that was opened. Start a name with `_` when the block does not use it (`with (_guard = Guard()) { ... }`), and the linter will not report it as unused.
- **The block must be in braces.**

When the block throws, every resource is closed and the block's error propagates. If a `close()` throws as well, the block's error still wins: it is the cause of the failure, and the close error is usually a consequence of it. The close error is not lost; when the block's error is an object (an `Error`), it is appended to its `suppressed` array. When the block ends without an error and a `close()` throws, that error propagates, after the other resources are closed.

```pleno
import { File } from "std:io";

let log = [];

class Connection {
    name: String;
    fn constructor(name: String) {
        this.name = name;
    }
    fn close() {
        log.push("closed " + this.name);
        throw Error("could not close " + this.name);
    }
}

try {
    with (a = Connection("a"), b = Connection("b")) {
        throw Error("request failed");
    }
} catch (e) {
    print(e.message);                                   // request failed
    print(e.suppressed.map((s) => s.message));         // ["could not close b", "could not close a"]
}
print(log);                                             // ["closed b", "closed a"]

fn firstLine(path: String): String {
    with (input = File.open(path)) {
        return input.readLines()[0];   // input is closed before the function returns
    }
}
```

`with (r = open()) { body }` is close to this `try`/`finally`, which is what to write for a value that needs a different cleanup call:

```pleno norun
let r = open();
try {
    body;
} finally {
    r.close();
}
```

The differences: a `finally` that throws replaces the body's error, while `with` keeps the body's error and records the close error in `suppressed`; and `with` checks for `close()` before the body runs, not after.

### Limits

| Limit | What happens |
| --- | --- |
| Calls nested more than 100,000 deep | `RecursionError: maximum call depth exceeded (100000 calls)`, which `catch` can handle |
| Calls from built-in functions such as `map`, `filter` or `sort` back into Pleno nested more than 200 deep | `RecursionError` (`200 nested callbacks`), which `catch` can handle |
| Brackets, blocks or `${...}` nested more than about 40 deep inside one expression | compile error `code nested too deeply` |
| More than 1,000 operators and calls in one statement | compile error `expression too long` |
| `--max-instructions`, `--max-memory` | `SandboxError`, which ends the program: no `catch` or `finally` runs (see [Security](./security-features.md)) |

```pleno
fn depth(n: Int): Int {
    return depth(n + 1);
}

try {
    depth(0);
} catch (e) {
    print(e.name);   // RecursionError
}
```

## What an error looks like

Every error a `catch` receives from the runtime or from `throw Error(...)` is an `Error` instance with two String fields:

- `message`: what went wrong, such as `division by zero`.
- `name`: the kind of error. It is `Error` unless the runtime says more: `TypeError` for a value that does not match a type annotation, `SecurityError` for an operation a sandbox or permission flag denies, `SandboxError` for an exceeded instruction or memory limit (which ends the program; see [Security](./security-features.md)), `RecursionError` when calls nest more than 100,000 deep (or calls from built-in functions such as `map` back into Pleno more than 200 deep), `AssertionError` for a failed `assert`. For a class that extends `Error`, it is the class name.

The `name` of a built-in error and the situations that throw it are part of the [compatibility promise](./compatibility.md); the wording of `message` is not, and can be improved in any release. Check `e.name` or catch by class, and do not parse messages. The messages quoted on these pages are today's wording.

Printing an error, or putting it in a string, gives `name: message`. Because runtime errors are `Error` instances, `catch (e: Error)` handles them too:

```pleno
fn half(n: Int): Int {
    return n / 2;
}

try {
    half("ten");
} catch (e: Error) {
    print(e);           // TypeError: parameter 'n' expected Int, got String
    print(e.name);      // TypeError
    print(e.message);   // parameter 'n' expected Int, got String
}

try {
    let zero = 0;
    print(1 / zero);
} catch (e) {
    print(`caught ${e}`);   // caught Error: division by zero
}

class NetworkError extends Error {}

try {
    throw NetworkError("connection refused");
} catch (e: Error) {
    print(e);           // NetworkError: connection refused
}
```

`throw` also accepts any other value, such as a string, and `catch` receives it unchanged; throwing an `Error` keeps errors uniform. For how much memory or how many instructions a program used, call `vmStats()` from `std:reflect`.

### "Did you mean" hints

When an error names something that does not exist, and a name close to it does, the message ends with `; did you mean '<name>'?`. This covers undefined variables (locals, parameters, globals, imports and built-ins in scope), unknown properties and methods (the type's built-in methods, methods added with `extend`, a map's keys, a module's exports, and a class's fields, methods and getters that the code may use, so a `private` member is suggested only inside its class), enum variants, unknown named arguments and unknown `std:` modules. `pleno check` and the language server add the same hints to their diagnostics, and `pleno check` warns about a name a `std:` module does not export.

```pleno
let items = [1, 2, 3];
try {
    items.pushh(4);
} catch (e) {
    print(e.message);   // cannot read property 'pushh' of Array; did you mean 'push'?
}
```

A hint is given only for a single closest name, at most a third of the name's length in edits away (at least one edit; case differences count as close). The text before the hint does not change, so code that checks the start of a message keeps working.

## Custom errors

Extend `Error` to carry extra information:

```pleno
class ValidationError extends Error {
    private field: String;

    constructor(message: String, field: String) {
        super(message);
        this.field = field;
    }

    public fn getField(): String {
        return this.field;
    }
}

fn validateAge(age: Int) {
    if (age < 0 || age > 150) {
        throw ValidationError("Invalid age", "age");
    }
}

try {
    validateAge(200);
} catch (e: ValidationError) {
    print(`${e.getField()}: ${e.message}`);   // age: Invalid age
}
```

## Result

A function that can fail in an expected way can return `Ok(value)` or `Err(error)` instead of throwing:

```pleno
fn divide(a: Float, b: Float): Result<Float, String> {
    if (b == 0.0) {
        return Err("Division by zero");
    }
    return Ok(a / b);
}

match (divide(10.0, 2.0)) {
    case Ok(value) => print("Result: " + value),   // Result: 5.0
    case Err(error) => print("Error: " + error)
}

let result = divide(1.0, 0.0);
print(result.isErr());          // true
print(result.unwrapOr(0.0));    // 0.0
```

`unwrap()` returns the value of an `Ok` and throws for an `Err`; `expect("message")` does the same with your message.

## Option

`Some(value)` or `None` says whether there is a value at all:

```pleno
let users = {"ada": "Ada Lovelace", "alan": "Alan Turing"};

fn findUser(login: String): Option<String> {
    if (users.containsKey(login)) {
        return Some(users[login]);
    }
    return None;
}

match (findUser("ada")) {
    case Some(name) => print("Found: " + name),   // Found: Ada Lovelace
    case None => print("Not found")
}

let missing = findUser("grace");
print(missing.isNone());               // true
print(missing.unwrapOr("nobody"));     // nobody
```
