# Functions

### 3.1 Function Declaration

#### Basic Function
```pleno
fn greet(name: String): String {
    return "Hello, " + name;
}

// Without a return type: returns null unless it returns a value
fn logMessage(msg: String) {
    print(msg);
}

// Single expression function
fn square(x: Int): Int => x * x;

// Multiple parameters
fn add(a: Int, b: Int): Int => a + b;
```

### 3.2 Default Parameters
```pleno
fn greet(name: String, greeting: String = "Hello"): String {
    return greeting + ", " + name;
}

greet("Alice");              // "Hello, Alice"
greet("Bob", "Hi");          // "Hi, Bob"
```

A default is evaluated each time the function is called without that argument, not once when the function is defined, so a default `[]` gives every call a new array (unlike Python). A default can use the parameters before it.

```pleno
fn addTo(item: Int, list: Int[] = []): Int[] {
    list.push(item);
    return list;
}
print(addTo(1), addTo(2));   // [1] [2]

fn rect(width: Int, height: Int = width): Int => width * height;
print(rect(3));              // 9
```

Calling a function with too few arguments throws `missing required argument`, and with too many `too many positional arguments`. This holds for named functions, lambdas and methods alike.

### 3.3 Named Parameters
```pleno
fn createUser(name: String, age: Int, active: Bool = true) {
    // implementation
}

// Call with named parameters
createUser(name: "Alice", age: 30);
createUser(age: 25, name: "Bob", active: false);
```

### 3.4 Variable Arguments
```pleno
fn sum(...numbers: Int[]): Int {
    let mut total = 0;
    for (num in numbers) {
        total += num;
    }
    return total;
}

sum(1, 2, 3, 4, 5);  // 15
```

The rest parameter collects the remaining arguments into an array. Its annotation (`Int[]` here) is checked like any other, so `sum(1, "a")` throws a `TypeError`.

### 3.4.1 Keyword-Only Parameters
```pleno
fn add(a: Int, *, b: Int): Int {
    return a + b;
}

add(1, b: 2); // OK
// add(1, 2) throws: keyword-only argument requires name
```

### 3.5 Higher-Order Functions
```pleno
fn applyOperation(x: Int, y: Int, op: (Int, Int) => Int): Int {
    return op(x, y);
}

let result = applyOperation(5, 3, fn(a, b) => a + b);

// Function type annotation
let operation: (Int, Int) => Int = fn(a, b) => a * b;
```

### 3.6 Closures
```pleno
fn makeCounter(): () => Int {
    let mut count = 0;
    return fn() => {
        count += 1;
        return count;
    };
}

let counter = makeCounter();
print(counter());  // 1
print(counter());  // 2
```

A closure, a nested `fn` and a method share the variables they use with the code around them; they do not copy them. An assignment made after the closure was created is visible inside it, and an assignment made inside it is visible outside. The same holds for parameters of the enclosing function.

```pleno
let mut verbose = false;
fn log(message: String) {
    if (verbose) { print(message); }
}
verbose = true;
log("now visible");      // now visible
```

Each iteration of a loop has its own loop variable, so closures created in different iterations keep different values. This holds for `for (x in ...)`, for the C-style `for (let i = 0; ...; i++)` and for destructuring patterns.

```pleno
let callbacks = [];
for (let i = 0; i < 3; i++) {
    callbacks.push(fn() => i);
}
print(callbacks.map(|f| => f()));   // [0, 1, 2]
```

### 3.7 Anonymous Functions & Lambdas
```pleno
// Anonymous function
let multiply = fn(x: Int, y: Int): Int {
    return x * y;
};

// Lambda (shorthand)
let square = |x| => x * x;
let add = |a, b| => a + b;

// Used in higher-order functions
[1, 2, 3, 4].map(|x| => x * 2);
```

Parameter annotations on anonymous functions and lambdas are checked when they are called, the same as on named functions: an `Int` passed for a `Float` becomes a `Float`, and optional (`Int?`) and union (`Int | String`) annotations accept any of their types. Parameters without an annotation are not checked and cost nothing.

```pleno
let half = (x: Float) => x / 2;
print(half(3));          // 1.5

let describe = fn(x: Int | String) => "got " + x;
print(describe("a"));    // got a

let multiply = fn(x: Int, y: Int): Int {
    return x * y;
};
try {
    multiply("2", 3);
} catch (e) {
    print(e.message);    // parameter 'x' expected Int, got String
}
```