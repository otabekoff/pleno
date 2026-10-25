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