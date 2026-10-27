# Decorators & Attributes

A decorator is written `@name` or `@name(args)` before a function or class. When `name` is a function in scope at that point, it is called with the function or class and its result replaces it; a decorator that returns `null` leaves the target unchanged. When `name` is not defined, the decorator does nothing at runtime and is kept as an annotation that [`Reflect.getAnnotations`](#attribute-access-via-reflection) can read. A misspelled decorator name is therefore not an error.

## Function Decorators

```pleno
// Define a decorator
fn logged(target: Function): Function {
    return fn(...args) {
        print("Calling: " + target.name);
        let result = target(...args);
        print("Result: " + result.toString());
        return result;
    };
}

// Apply decorator
@logged
fn calculate(x: Int, y: Int): Int {
    return x + y;
}

calculate(2, 3);
// Output:
// Calling: calculate
// Result: 5
```

## Multiple Decorators

Decorators are applied bottom-up. In this example none of the three names is defined, so they are only annotations:

```pleno
@authenticated   // Applied third
@cached          // Applied second
@logged          // Applied first
fn getUserData(userId: Int): User {
    // implementation
}
```

## Class Decorators

```pleno
@serializable
@observable
class User {
    public name: String;
    public age: Int;
}
```

## Decorator with Arguments

```pleno
fn rateLimit(maxCalls: Int, windowMs: Int) {
    return fn(target: Function): Function {
        let mut calls = 0;
        return fn(...args) {
            if (calls >= maxCalls) {
                throw Error("Rate limit exceeded");
            }
            calls = calls + 1;
            return target(...args);
        };
    };
}

@rateLimit(100, 60000)  // 100 calls per minute
fn apiEndpoint() {
    // ...
}
```

## Built-in Annotations

These names are handled by the compiler instead of being called:

| Annotation     | Effect                                                                                                 |
| -------------- | ------------------------------------------------------------------------------------------------------ |
| `@pure`        | Checks some side effects at runtime; experimental, see [Pure functions](./functional-programming.md) |
| `@inline`      | Lets the optimizer inline calls to the function; its return annotation is not checked                  |
| `@no_optimize` | Keeps the optimizer (constant folding, dead code removal) away from the function                      |
| `@hot`         | Recorded on the function; no effect                                                                   |
| `@deprecated`  | Warns when the function or method is used; see below                                                   |

`@deprecated` or `@deprecated("use add instead")` marks a function or method that is going away. The first call to it prints `warning: plus is deprecated: use add instead` to standard error (once per name), and `pleno check` and the editor report each call to a deprecated top-level function as a warning. The function keeps working.

```pleno
@deprecated("use add instead")
fn plus(a, b) { return a + b; }

fn add(a, b) { return a + b; }

print(plus(1, 2));   // 3, after "warning: plus is deprecated: use add instead"
```

Other names, such as `@override` and `@sealed`, are ordinary annotations: they do nothing unless you define a function by that name. In particular `@override` does not check that a parent method exists, and `@sealed` does not prevent subclasses. Abstract classes and methods use the `abstract` keyword (`abstract class`, `abstract fn`), not an annotation; see [Abstract classes](./OOP.md#abstract-classes).

## Attribute Access via Reflection

```pleno
import { Reflect } from "std:reflect";

@Entity
@Table("users")
class User {
    @Column("id")
    public id: Int;
}

let annotations = Reflect.getAnnotations(User);
print(annotations);   // [{"name": "Entity", "args": []}, {"name": "Table", "args": ["users"]}]
```

`Reflect.getAnnotations` returns the annotations of a class or enum (or of an instance's class). Annotations on fields, such as `@Column("id")` here, and on functions are accepted but not returned.
