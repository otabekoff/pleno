# Core Syntax & Fundamentals

### 1.1 Comments
```pleno
// Single-line comment

/* Multi-line comment
   spans multiple lines */

/// Documentation comment for functions, classes, modules
/// Can be extracted by documentation generators
```

### 1.2 Variables & Constants

#### Variables
```pleno
let x = 10;                    // Type inference
let name: String = "Alice";    // Explicit type
let mut counter = 0;           // Explicitly mutable
```

A binding without `mut` cannot be reassigned from anywhere, including from a function or closure that captures it: `let x = 1; fn f() { x = 2; }` is a compile error (`cannot assign to immutable binding`). The same holds for `++` and `--`. Names declared with `fn`, `class`, `enum` and `interface` cannot be assigned either (`cannot assign to function 'f'`); use `let mut` for a name that should point to different values over time.

#### Destructuring
```pleno
let (name, age) = ("Alice", 30);       // Tuple
let [first, ...rest] = [1, 2, 3];      // Array, with a rest element
let mut (x, _) = (1, 2);               // `_` skips a value
print(name, age, first, rest, x);      // Alice 30 1 [2, 3] 1
```

See [Destructuring](./data-structures#destructuring) for the rules.

#### Constants (truly immutable)
```pleno
const PI = 3.14159;            // Compile-time constant
const MAX_USERS: Int = 1000;   // Typed constant

// Constants cannot be reassigned - enforced at runtime
const list = [1, 2, 3];
// list = [4, 5, 6];  // ERROR: Cannot reassign constant
// list.push(4);      // ERROR: Cannot mutate constant value
```

#### Deep Immutability
```pleno
const deep obj = {
    name: "Alice",
    scores: [95, 87, 92]
};
// obj.name = "Bob";           // ERROR
// obj.scores.push(88);         // ERROR
// obj.scores[0] = 100;         // ERROR
```

### 1.3 Type System

#### Primitive Types
```text
Int      // 64-bit signed integer
Float    // 64-bit floating point
String   // UTF-8 string
Bool     // true or false
Char     // Single Unicode character
Null     // null type
Void     // No return value
Any      // Dynamic type (opt-out of type checking)
```

Ints and Floats compare by value: `5.0 == 5` is `true` and `5.5 == 5` is `false`. The same holds in sets (`Set([5, 5.0])` has one element), `contains`, `indexOf` and `assert.equal`.

```pleno
print(10 / 2 == 5);           // true
print(5.5 != 5);              // true
print(Set([5, 5.0]).size());  // 1
```

#### Type Annotations
```pleno
let age: Int = 25;
let price: Float = 99.99;
let active: Bool = true;
let data: Any = "flexible";

// Function with type annotations
fn calculate(x: Int, y: Float): Float {
    return x * y;
}
```

Annotations are checked when the program runs: on parameters, `let` bindings, assignments to annotated variables, class fields and return values. A value of the wrong type throws a `TypeError`:

```pleno
fn label(count: Int): String {
    return count;   // TypeError: label returned Int, expected String
}
```

A return annotation is checked at every `return`, and also when a function ends without one: it then returns `null`, which an `Int` annotation rejects (`TypeError: f returned Null, expected Int`) and `Int?` accepts. For an `async fn` the annotation describes the value the promise resolves to, so `async fn f(): Int` and `async fn f(): Promise<Int>` both check for an `Int`. Not checked: `Void` and `Any` returns, returns whose annotation mentions a type parameter (`T`, `T[]`), the type arguments of a user generic class (`Box<Int>` checks for a `Box`), setters, constructors and functions marked `@inline`. The check runs at the `return` statement, so a `try`/`catch` around that `return` in the same function catches its `TypeError`.

**Int and Float.** An `Int` is accepted where a `Float` is expected, and it becomes a `Float` there: in `fn half(x: Float)`, `half(7)` binds `x` to `7.0`, so `x / 2` is `3.5`. The same applies to `let`, assignments, fields and return values, and to `Float?` and unions such as `Float | String`. In a union that also allows `Int` (`Int | Float`, `Number`, `Any`), an `Int` stays an `Int`. Collections are checked but not copied: a `Float[]` (or `Map<String, Float>`) accepts `[1, 2.5]` and keeps the `1` an `Int`. A `Float` prints without a trailing `.0` when it is whole (`print(3.0)` prints `3`), so the conversion shows in `typeof` and in division.

```pleno
fn half(x: Float): Float {
    return x / 2;
}
print(half(7));             // 3.5
let ratio: Float = 1;
print(typeof(ratio));       // Float
let either: Int | Float = 1;
print(typeof(either));      // Int
```

#### Type Inference
```pleno
let count = 42;              // Inferred as Int
let ratio = 3.14;            // Inferred as Float
let message = "Hello";       // Inferred as String
let mixed = null;            // Inferred as Null (can be reassigned)
```

#### Union Types
```pleno
let mut result: Int | String = 42;
result = "error";  // Valid

fn process(value: Int | Float | String): String {
    return value.toString();
}
```

#### Optional Types
```pleno
let optional: Int? = null;   // Can be Int or null
let username: String? = "alice";

// Safe navigation operator
let length = username?.length;  // Returns Int? (null if username is null)

// Null coalescing
let displayName = username ?? "Guest";

// Forced unwrap (runtime error if null)
let definite = username!;  // Use with caution
```

#### Type Guards
```pleno
fn processValue(val: Int | String) {
    if (val is Int) {
        // val is narrowed to Int here
        print(val * 2);
    } else {
        // val is narrowed to String here
        print(val.toUpperCase());
    }
}
```


### 1.4 TypeOf Operator
```pleno
let x = 42;
print(typeof(x));  // "Int"
```