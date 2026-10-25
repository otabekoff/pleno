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

## What an error looks like

Every error a `catch` receives from the runtime or from `throw Error(...)` is an `Error` instance with two String fields:

- `message`: what went wrong, such as `division by zero`.
- `name`: the kind of error. It is `Error` unless the runtime says more: `TypeError` for a value that does not match a type annotation, `SecurityError` for an operation a sandbox or permission flag denies, `SandboxError` for an exceeded instruction or memory limit (which ends the program; see [Security](./security-features.md)), `RecursionError` when calls nest more than 100,000 deep (or calls from built-in functions such as `map` back into Pleno more than 200 deep), `AssertionError` for a failed `assert`. For a class that extends `Error`, it is the class name.

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
    case Ok(value) => print("Result: " + value),   // Result: 5
    case Err(error) => print("Error: " + error)
}

let result = divide(1.0, 0.0);
print(result.isErr());          // true
print(result.unwrapOr(0.0));    // 0
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
