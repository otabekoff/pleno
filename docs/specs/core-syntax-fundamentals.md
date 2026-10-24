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

#### Variables (mutable)
```pleno
let x = 10;                    // Type inference
let name: String = "Alice";    // Explicit type
let mut counter = 0;           // Explicitly mutable
```

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