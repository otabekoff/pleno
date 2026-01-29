# Functional Programming Features

### 6.1 First-Class Functions

```pleno
// Functions as values
let operation = fn(x: Int, y: Int): Int => x + y;

// Functions as parameters
fn applyTwice(f: (Int) => Int, x: Int): Int {
    return f(f(x));
}

// Functions as return values
fn makeMultiplier(factor: Int): (Int) => Int {
    return |x| => x * factor;
}

let double = makeMultiplier(2);
print(double(5));  // 10
```

#### Default and Named Arguments

```pleno
fn sum(a, b = 2, c = 3) {
    return a + b + c;
}

sum(1);                // 6
sum(1, c = 10);        // 13
sum(b = 5, a = 2);     // 10
```

### 6.2 Pure Functions

```pleno
// Pure function annotation (compiler enforces purity)
@pure
fn calculateArea(width: Float, height: Float): Float {
    return width * height;
    // Cannot modify external state or have side effects
}
```

### 6.3 Immutable Data Structures

```pleno
// Immutable list
const immutable list = [1, 2, 3];
let newList = list.with(4);  // Creates new list [1, 2, 3, 4]

// Immutable map
const immutable config = {
    "host": "localhost",
    "port": 8080
};
let newConfig = config.with("timeout", 30);
```

### 6.4 Pipeline Operator

```pleno
// Chain operations with |>
let result = [1, 2, 3, 4, 5]
    |> filter(|x| => x % 2 == 0)
    |> map(|x| => x * x)
    |> reduce(|acc, x| => acc + x, 0);

// Equivalent to:
// reduce(map(filter([1,2,3,4,5], fn), fn), fn, 0)
```

### 6.5 Partial Application & Currying

```pleno
fn add(a: Int, b: Int, c: Int): Int {
    return a + b + c;
}

// Partial application
let add5 = add(5, _, _);  // Placeholder for later arguments
print(add5(3, 2));        // 10

// Currying
fn curriedAdd(a: Int): (Int) => (Int) => Int {
    return |b| => |c| => a + b + c;
}

let result = curriedAdd(5)(3)(2);  // 10
```

### 6.6 Pattern Matching on Data

```pleno
fn processData(data: Any) {
    match (data) {
        case [] => print("Empty array"),
        case [single] => print("Single element: " + single),
        case [first, ...rest] => {
            print("First: " + first);
            print("Rest: " + rest);
        },
        case {name, age} => print("Person: " + name + ", " + age),
        case _ => print("Unknown")
    }
}
```