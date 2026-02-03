# Decorators & Attributes

Decorators are functions that modify classes, methods, or functions at runtime.

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

Decorators are applied bottom-up:

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
        let calls = 0;
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

## Built-in Decorators

| Decorator     | Description                    |
| ------------- | ------------------------------ |
| `@deprecated` | Mark as deprecated             |
| `@override`   | Ensure method overrides parent |
| `@abstract`   | Mark class/method as abstract  |
| `@sealed`     | Prevent inheritance            |

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
// [{ name: "Entity", args: [] }, { name: "Table", args: ["users"] }]
```
