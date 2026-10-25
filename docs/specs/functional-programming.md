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
@pure
fn calculateArea(width: Float, height: Float): Float {
    return width * height;
}
print(calculateArea(2, 3));   // 6

@pure
fn noisy(x: Int): Int {
    print("side effect");     // throws: pure function cannot have side effects
    return x;
}
```

**Experimental.** `@pure` is not covered by the [compatibility promise](./compatibility.md): its check will change. Today it is checked while the function runs, not by the compiler. Inside a `@pure` function, calling `print` or `input`, assigning to a field or an index, and changing a class throw `pure function cannot have side effects`. The check misses some side effects and rejects some code that has none:

- Not detected: standard library calls such as `File.write`, methods that change their receiver (`items.push(x)` on an argument), and assigning to a variable of an enclosing function or of the main file.
- Rejected although harmless: creating a non-empty array or map literal, such as `[1, 2]` or `{"a": 1}`, inside the function.

### 6.3 Immutable Data Structures

```pleno
// Immutable list (`deep` also freezes nested values)
const deep list = [1, 2, 3];
let newList = list.with(4);  // A new list [1, 2, 3, 4], frozen like the original

// Immutable map
const deep config = {
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