# std:core

Core types and utilities available in every Pleno program.

## Import

These types are available globally without explicit import.

## Result Type

The `Result<T, E>` type represents either success (`Ok`) or failure (`Err`).

### `Ok<T>(value: T): Result<T, E>`

Creates a successful result containing `value`.

```pleno
fn divide(a: Float, b: Float): Result<Float, String> {
    if (b == 0.0) {
        return Err("Division by zero");
    }
    return Ok(a / b);
}

print(divide(10.0, 4.0).unwrap());  // 2.5
```

### `Err<E>(error: E): Result<T, E>`

Creates a failure result containing `error`.

```pleno
let result = Err("File not found");
print(result.isErr());  // true
```

### Result Methods

- `isOk()` returns `true` for `Ok`.
- `isErr()` returns `true` for `Err`.
- `unwrap()` returns the value, or throws `called unwrap on Err`.
- `unwrapOr(default)` returns the value, or `default` for `Err`.
- `expect(message)` returns the value, or throws `message`.

```pleno
let good = Ok(42);
let bad = Err("timeout");

print(good.isOk());        // true
print(bad.isErr());        // true
print(good.unwrap());      // 42
print(bad.unwrapOr(0));    // 0
print(good.expect("no value"));  // 42
```

---

## Option Type

The `Option<T>` type represents an optional value.

### `Some<T>(value: T): Option<T>`

Creates an option containing a value.

```pleno
let users = ["Ada", "Grace"];

fn findUser(id: Int): Option<String> {
    if (id >= 0 && id < users.length) {
        return Some(users[id]);
    }
    return None;
}

print(findUser(0).unwrapOr("nobody"));  // Ada
print(findUser(9).unwrapOr("nobody"));  // nobody
```

### `None: Option<T>`

Represents the absence of a value.

```pleno
let empty: Option<String> = None;
print(empty.isNone());  // true
```

### Option Methods

- `isSome()` returns `true` if there is a value.
- `isNone()` returns `true` if there is no value.
- `unwrap()` returns the value, or throws `called unwrap on None`.
- `unwrapOr(default)` returns the value, or `default` for `None`.
- `expect(message)` returns the value, or throws `message`.

```pleno
let name = Some("Ada");
let missing: Option<String> = None;

print(name.isSome());               // true
print(missing.isNone());            // true
print(name.unwrap());               // Ada
print(missing.unwrapOr("unknown")); // unknown
print(name.expect("no name"));      // Ada
```

---
