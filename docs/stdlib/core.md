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
```

### `Err<E>(error: E): Result<T, E>`

Creates a failure result containing `error`.

```pleno
let result = Err("File not found");
```

### Result Methods

```pleno
result.isOk();           // true if Ok
result.isErr();          // true if Err
result.unwrap();         // Get value or panic
result.unwrapOr(default); // Get value or default
result.expect("msg");    // Get value or panic with message
```

---

## Option Type

The `Option<T>` type represents an optional value.

### `Some<T>(value: T): Option<T>`

Creates an option containing a value.

```pleno
fn findUser(id: Int): Option<User> {
    if (users.has(id)) {
        return Some(users.get(id));
    }
    return None;
}
```

### `None: Option<T>`

Represents the absence of a value.

```pleno
let empty: Option<String> = None;
```

### Option Methods

```pleno
option.isSome();         // true if has value
option.isNone();         // true if empty
option.unwrap();         // Get value or panic
option.unwrapOr(default); // Get value or default
```

---
