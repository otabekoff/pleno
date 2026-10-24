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

An error that nothing catches stops the program with its message and a stack trace.

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
