# Core Syntax & Fundamentals

### 1.1 Comments
```pleno
// Single-line comment

/* Multi-line comment
   spans multiple lines */

/// Documentation comment for functions, classes, modules
/// `pleno doc` turns these into HTML
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

const list = [1, 2, 3];
// list = [4, 5, 6];  // compile error: cannot assign to immutable binding
// list.push(4);      // runtime error: cannot modify immutable array
```

A `const` cannot be reassigned (a compile error), and the array, map or object it holds cannot be changed (a runtime error). Values nested inside it, such as an array stored in a `const` map, can still change; `const deep` freezes those too.

#### Deep Immutability
`const deep` freezes the value and everything reachable from it. Changing any part throws `cannot modify immutable map` (or `array`).

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

Arrays, tuples, maps and sets compare by contents. Arrays and tuples are equal when they have the same elements in the same order (a named tuple's names must match too), maps when they have the same keys with equal values in any order, and sets when they have the same elements. Nested values compare the same way, and values that contain themselves compare without looping. Class instances are equal only to themselves, unless their class defines `operator ==` (see [Operator overloading](./advanced-features.md)), which is also used for instances inside arrays, tuples and maps. `contains` and `indexOf` use the same rules.

```pleno
print([1, [2, 3]] == [1, [2, 3]]);       // true
print({"a": 1, "b": 2} == {"b": 2, "a": 1}); // true
print((1, "x") == (1, "x"));             // true
print([1, 2] == [2, 1]);                 // false
print([[1]].contains([1]));              // true
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

A return annotation is checked at every `return`, and also when a function ends without one: it then returns `null`, which an `Int` annotation rejects (`TypeError: f returned Null, expected Int`) and `Int?` accepts. For an `async fn` the annotation describes the value the promise resolves to, so `async fn f(): Int` and `async fn f(): Promise<Int>` both check for an `Int`. The check runs at the `return` statement, so a `try`/`catch` around that `return` in the same function catches its `TypeError`. Functions marked `@inline` are checked the same way where they are inlined.

A `Void` function returns `null`: `return;` and `return null;` are accepted, `return 1;` throws `TypeError: f returned Int, expected Void`, and the value of the last expression in its body (or of the body of `fn(): Void => list.pop()`) is not returned.

A return annotation that mentions a type parameter (`T`, `T[]`, `T?`) is checked against the type argument: with `fn first<T>(xs: T[]): T`, `first<Int>([1, 2])` must return an `Int`. When the type argument was inferred (`first([1, 2])`), `T` accepts any value, or any value that satisfies its constraint (`T: Named`), so a right program is never rejected. A class's type parameter in a method is bound to the instance's type arguments (`Box<Int>(1)`), and accepts any value for an instance made without them (`Box(1)`). The return annotation `Box<Int>` of a class declared in the same file checks the type arguments, as a parameter does (see [Generics](./generics.md)); for a class imported from another module, and for `Box<Int>?`, only the class is checked. Not checked: `Any` returns, `Self`, setters, constructors, and annotations that mention the type parameter of an extension (`extend <T> Array<T>`) or of a function enclosing a class declaration (a method of `class Local` inside `fn make<T>()` cannot see `T`).

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
let mut mixed = null;        // Null for now; `mut` allows another value later
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