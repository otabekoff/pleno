# Pleno Programming Language

## Complete Language Specification v1.0

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices

---

## 1. Core Syntax & Fundamentals ✅

### 1.1 Comments ✅

```pleno
// Single-line comment

/* Multi-line comment
   spans multiple lines */

/// Documentation comment for functions, classes, modules
/// Can be extracted by documentation generators
```

### 1.2 Variables & Constants ✅

#### Variables (mutable) ✅

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

#### Deep Immutability ✅

```pleno
const deep obj = {
    name: "Alice",
    scores: [95, 87, 92]
};
// obj.name = "Bob";           // ERROR
// obj.scores.push(88);         // ERROR
// obj.scores[0] = 100;         // ERROR

// Note: 'let deep' is also supported for runtime-initialized immutability.
let deep data = fetchData();
```

### 1.3 Type System

#### Primitive Types

```pleno
Int      // 64-bit signed integer
Float    // 64-bit floating point
String   // UTF-8 string
Bool     // true or false
Char     // Single Unicode character, e.g., 'A', 'π', '\n'
Null     // null type
Void     // No return value
Any      // Dynamic type (opt-out of type checking)
```

#### Char Literals ✅

Characters are enclosed in single quotes:

```pleno
let a = 'A';
let pi = 'π';
let newline = '\n';
let tab = '\t';
```

Unlike some other languages, `Char` is a distinct primitive type in Pleno, not just a small integer or a single-character string.

````

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
````

#### Type Inference

```pleno
let count = 42;              // Inferred as Int
let ratio = 3.14;            // Inferred as Float
let message = "Hello";       // Inferred as String
let mixed = null;            // Inferred as Null (can be reassigned)
```

#### Union Types

```pleno
let result: Int | String = 42;
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

### 1.4 TypeOf Operator ✅

```pleno
let x = 42;
print(typeof(x));  // "Int"
```

### 1.5 Increment & Decrement ✅

```pleno
let mut i = 0;
i++;    // Post-increment (returns 0, i becomes 1)
++i;    // Pre-increment (returns 2, i becomes 2)
i--;    // Post-decrement (returns 2, i becomes 1)
--i;    // Pre-decrement (returns 0, i becomes 0)

// Also works on properties and indexes
obj.prop++;
arr[0]--;
```

### 1.6 String Interpolation ✅

Strings support interpolation using the `${expression}` syntax. Expressions within `${}` are evaluated and their string representation is embedded in the literal.

```pleno
let name = "World";
print("Hello, ${name}!"); // Hello, World!

let x = 10, y = 20;
print("Sum: ${x + y}");   // Sum: 30

// Nested expressions and quotes are supported
print("Result: ${"nested " + name}");
```

---

## 2. Control Flow ✅

### 2.1 Conditionals ✅

#### If-Else

```pleno
if (condition) {
    // code
} else if (otherCondition) {
    // code
} else {
    // code
}

// If as expression
let status = if (score >= 60) "Pass" else "Fail";
```

#### Pattern Matching (Match Expression) ✅

```pleno
match (value) {
    case 0 => print("zero"),
    case 1, 2, 3 => print("small"),
    case x if x > 100 => print("large: " + x),
    case String s => print("string: " + s),
    case _ => print("other")  // Default case
}

// Match as expression
let category = match (age) {
    case x if x < 13 => "child",
    case x if x < 20 => "teen",
    case x if x < 65 => "adult",
    case _ => "senior"
};
```

### 2.2 Loops ✅

#### For Loop

```pleno
// Traditional for ✅
for (let i = 0; i < 10; i++) {
    print(i);
}

// For-in (iteration) ✅
for (item in collection) {
    print(item);
}

// For-in with index ✅
for (index, item in collection) {
    print(index + ": " + item);
}

// Range-based ✅
for (i in 0..10) {      // Exclusive end
    print(i);
}

for (i in 0..=10) {     // Inclusive end
    print(i);
}
```

#### While Loop ✅

```pleno
while (condition) {
    // code
}

// Do-while ✅
do {
    // code
} while (condition);
```

#### Loop Control ✅

```pleno
break;        // Exit loop
continue;     // Skip to next iteration
break label;  // Break from labeled outer loop ✅

outer: for (i in 0..10) {
    for (j in 0..10) {
        if (i * j > 50) break outer;
    }
}
```

---

## 3. Functions ✅

### 3.1 Function Declaration ✅

#### Basic Function

```pleno
fn greet(name: String): String {
    return "Hello, " + name;
}

// Without return type (inferred as Void)
fn logMessage(msg: String) {
    print(msg);
}

// Single expression function
fn square(x: Int): Int => x * x; ✅

// Multiple parameters
fn add(a: Int, b: Int): Int => a + b; ✅
```

### 3.2 Default Parameters ✅

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
    let total = 0;
    for (num in numbers) {
        total += num;
    }
    return total;
}

sum(1, 2, 3, 4, 5);  // 15
```

### 3.4.1 Keyword-Only Parameters

```pleno
fn add(a: Int, *, b: Int): Int {
    return a + b;
}

add(1, b: 2); // OK
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
    let count = 0;
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

---

## 4. Object-Oriented Programming ✅

### 4.1 Class Definition ✅

```pleno
class Person {
    // Private fields (truly encapsulated)
    private name: String;
    private age: Int;

    // Protected field (accessible in subclasses)
    protected id: String;

    // Public field
    public active: Bool;

    // Constructor
    constructor(name: String, age: Int) {
        this.name = name;
        this.age = age;
        this.id = generateId();
        this.active = true;
    }

    // Public method
    public fn greet(): String {
        return "Hello, I'm " + this.name;
    }

    // Private method
    private fn generateId(): String {
        return "ID_" + Math.random().toString();
    }

    // Getter
    public fn getName(): String {
        return this.name;
    }

    // Setter with validation
    public fn setAge(newAge: Int) {
        if (newAge >= 0 && newAge <= 150) {
            this.age = newAge;
        } else {
            throw Error("Invalid age");
        }
    }
}

// Usage
let person = Person("Alice", 30);
print(person.greet());
// print(person.name);  // ERROR: Cannot access private field
print(person.active);   // OK: Public field
```

### 4.2 Properties (Getters/Setters)

```pleno
class Circle {
    private radius: Float;

    constructor(radius: Float) {
        this.radius = radius;
    }

    // Property with getter
    property area: Float {
        get {
            return Math.PI * this.radius * this.radius;
        }
    }

    // Property with getter and setter
    property diameter: Float {
        get {
            return this.radius * 2;
        }
        set(value) {
            this.radius = value / 2;
        }
    }
}

let circle = Circle(5.0);
print(circle.area);      // 78.54 (computed)
circle.diameter = 20.0;  // Sets radius to 10
```

### 4.3 Inheritance

```pleno
class Animal {
    protected name: String;

    constructor(name: String) {
        this.name = name;
    }

    public fn speak(): String {
        return "Some sound";
    }
}

class Dog extends Animal {
    private breed: String;

    constructor(name: String, breed: String) {
        super(name);  // Call parent constructor
        this.breed = breed;
    }

    // Override parent method
    @override
    public fn speak(): String {
        return "Woof! I'm " + this.name;
    }

    public fn getBreed(): String {
        return this.breed;
    }
}

let dog = Dog("Buddy", "Golden Retriever");
print(dog.speak());  // "Woof! I'm Buddy"
```

### 4.4 Abstract Classes

```pleno
abstract class Shape {
    protected color: String;

    constructor(color: String) {
        this.color = color;
    }

    // Abstract method (must be implemented by subclasses)
    abstract fn area(): Float;

    // Concrete method
    public fn describe(): String {
        return "A " + this.color + " shape";
    }
}

class Rectangle extends Shape {
    private width: Float;
    private height: Float;

    constructor(color: String, width: Float, height: Float) {
        super(color);
        this.width = width;
        this.height = height;
    }

    @override
    fn area(): Float {
        return this.width * this.height;
    }
}

// let shape = Shape("red");  // ERROR: Cannot instantiate abstract class
let rect = Rectangle("blue", 10.0, 5.0);
print(rect.area());  // 50.0
```

### 4.5 Interfaces

```pleno
interface Drawable {
    fn draw(): Void;
    fn getColor(): String;
}

interface Resizable {
    fn resize(factor: Float): Void;
}

// Multiple interface implementation
class Canvas implements Drawable, Resizable {
    private color: String;
    private size: Float;

    constructor(color: String, size: Float) {
        this.color = color;
        this.size = size;
    }

    fn draw(): Void {
        print("Drawing canvas");
    }

    fn getColor(): String {
        return this.color;
    }

    fn resize(factor: Float): Void {
        this.size *= factor;
    }
}
```

### 4.6 Static Members

```pleno
class MathUtils {
    // Static constant
    public static const PI: Float = 3.14159;

    // Static field
    private static instanceCount: Int = 0;

    constructor() {
        MathUtils.instanceCount++;
    }

    // Static method
    public static fn square(x: Float): Float {
        return x * x;
    }

    public static fn getInstanceCount(): Int {
        return MathUtils.instanceCount;
    }
}

// Usage without instance
print(MathUtils.PI);
print(MathUtils.square(5.0));
MathUtils.version = "1.0";
print(MathUtils.version);
```

### 4.7 Access Modifiers

- `public` - Accessible everywhere (default for methods)
- `private` - Accessible only within the class
- `protected` - Accessible within class and subclasses
- `internal` - Accessible within the same module

Access modifiers are enforced for methods and class fields (including static fields), including `internal`.

```pleno
class Example {
    public publicField: Int;
    private privateField: Int;
    protected protectedField: Int;
    internal internalField: Int;
}
```

---

## 5. Data Structures ✅

### 5.1 Arrays ✅

```pleno
// Array literal
let numbers: Int[] = [1, 2, 3, 4, 5];
let mixed: Any[] = [1, "two", 3.0, true];

// Array operations
numbers.push(6);           // Add to end
numbers.pop();             // Remove from end
numbers.unshift(0);        // Add to start
numbers.shift();           // Remove from start
numbers.insert(2, 99);     // Insert at index
numbers.remove(1);         // Remove at index

// Array methods
numbers.length;            // Get length
numbers.isEmpty();         // Check if empty
numbers.contains(3);       // Check if contains element
numbers.indexOf(4);        // Find index of element
numbers.reverse();         // Reverse in place
numbers.sort();            // Sort in place

// Functional methods
numbers.map(|x| => x * 2);
numbers.filter(|x| => x > 3);
numbers.reduce(|acc, x| => acc + x, 0);
numbers.forEach(|x| => print(x));

// Array slicing
let slice = numbers[1..4];    // Elements 1, 2, 3
let slice2 = numbers[2..];    // From index 2 to end
let slice3 = numbers[..3];    // From start to index 3
```

### 5.2 Maps (Dictionaries) ✅

```pleno
// Map literal
let ages: Map<String, Int> = {
    "Alice": 30,
    "Bob": 25,
    "Charlie": 35
};

// Type-inferred map
let scores = {
    "math": 95,
    "science": 88,
    "english": 92
};

// Map operations
ages.put("David", 28);        // Add/update entry
ages.remove("Bob");           // Remove entry
ages.get("Alice");            // Get value (returns Int?)
ages.containsKey("Charlie");  // Check if key exists
ages.keys();                  // Get all keys
ages.values();                // Get all values
ages.entries();               // Get all key-value pairs
ages.size();                  // Get size
ages.isEmpty();               // Check if empty

// Safe access
let age = ages["Alice"] ?? 0;  // Default value if not found

// Iteration
for (key, value in ages) {
    print(key + ": " + value);
}
```

### 5.3 Sets

```pleno
let uniqueNumbers: Set<Int> = {1, 2, 3, 4, 5};

// Set operations
uniqueNumbers.add(6);
uniqueNumbers.remove(3);
uniqueNumbers.contains(4);
uniqueNumbers.size();
uniqueNumbers.isEmpty();

// Set operations
let set1 = {1, 2, 3};
let set2 = {3, 4, 5};

set1.union(set2);           // {1, 2, 3, 4, 5}
set1.intersection(set2);    // {3}
set1.difference(set2);      // {1, 2}
```

### 5.4 Tuples

```pleno
// Tuple with different types
let person: (String, Int, Bool) = ("Alice", 30, true);

// Destructuring
let (name, age, active) = person;

// Access by index
print(person.0);  // "Alice"
print(person.1);  // 30

// Named tuples
let point: (x: Float, y: Float) = (x: 10.5, y: 20.3);
print(point.x);   // 10.5
```

### 5.5 Ranges

```pleno
let range1 = 0..10;      // 0 to 9 (exclusive end)
let range2 = 0..=10;     // 0 to 10 (inclusive end)
let range3 = 5..;        // 5 to infinity
let range4 = ..10;       // negative infinity to 9

// Range methods
range1.contains(5);      // true
range1.toArray();        // [0, 1, 2, ..., 9]

// Iteration
for (i in 1..=5) {
    print(i);  // 1, 2, 3, 4, 5
}

// Unbounded ranges iterate until you break
for (i in 5..) {
    if (i > 10) { break; }
    print(i);
}
```

---

## 6. Functional Programming Features ✅

### 6.1 First-Class Functions ✅

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

### 6.4 Pipeline Operator ✅

```pleno
// Chain operations with |>
let result = [1, 2, 3, 4, 5]
    |> filter(|x| => x % 2 == 0)
    |> map(|x| => x * x)
    |> reduce(|acc, x| => acc + x, 0);

// Equivalent to:
// reduce(map(filter([1,2,3,4,5], fn), fn), fn, 0)
```

### 6.5 Partial Application & Currying ✅

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

---

## 7. Error Handling ✅

### 7.1 Try-Catch

```pleno
try {
    // Code that might throw
    let result = riskyOperation();
} catch (e: NetworkError) {
    print("Network error: " + e.message);
} catch (e: ValidationError) {
    print("Validation error: " + e.message);
} catch (e: Error) {
    print("General error: " + e.message);
} finally {
    // Always executes
    cleanup();
}
```

### 7.2 Custom Exceptions

```pleno
class ValidationError extends Error {
    private field: String;

    constructor(message: String, field: String) {
        super(message);
        this.field = field;
    }

    public fn getField(): String {
        return this.field;
    }
}

fn validateAge(age: Int) {
    if (age < 0 || age > 150) {
        throw ValidationError("Invalid age", "age");
    }
}
```

### 7.3 Result Type (Functional Error Handling)

```pleno
// Result type for functions that might fail
fn divide(a: Float, b: Float): Result<Float, String> {
    if (b == 0.0) {
        return Err("Division by zero");
    }
    return Ok(a / b);
}

// Using Result
let result = divide(10.0, 2.0);
match (result) {
    case Ok(value) => print("Result: " + value),
    case Err(error) => print("Error: " + error)
}

// Result methods
let value = result.unwrap();           // Panics if Err
let value = result.unwrapOr(0.0);     // Default value if Err
let value = result.expect("Failed");   // Panics with message if Err
```

### 7.4 Option Type

```pleno
fn findUser(id: Int): Option<User> {
    // Returns Some(user) or None
    if (userExists(id)) {
        return Some(getUser(id));
    }
    return None;
}

// Using Option
let user = findUser(123);
match (user) {
    case Some(u) => print("Found: " + u.name),
    case None => print("Not found")
}

// Option methods
user.unwrap();              // Panics if None
user.unwrapOr(defaultUser); // Default if None
user.isSome();              // Check if Some
user.isNone();              // Check if None
```

---

## 8. Modules & Imports ✅

Current implementation supports:

```pleno
import "path/to/module.pln";
import ModuleName from "path/to/module.pln";
import { symbol, other as alias } from "path/to/module.pln";
import * as ModuleName from "path/to/module.pln";
```

Exports are limited to `export` on `fn`, `class`, `let`, and `const` declarations plus `export { name as alias };`. Import returns a module namespace map (accessed via `ModuleName.field`). Module declarations and structured imports are implemented for string paths; relative paths and `sources/` lookups are supported. Package/registry resolution is not implemented yet.

### 8.1 Module Definition

```pleno
// File: math_utils.pln
module MathUtils;

// Public export
export fn square(x: Float): Float {
    return x * x;
}

export const PI: Float = 3.14159;

// Private (not exported)
fn helperFunction(): Void {
    // Only available within module
}

export class Calculator {
    public fn add(a: Float, b: Float): Float {
        return a + b;
    }
}
```

### 8.2 Importing

```pleno
// Import entire module
import MathUtils;
MathUtils.square(5.0);

// Import specific items
import { square, PI } from MathUtils;
square(5.0);

// Import with alias
import { square as sq } from MathUtils;
sq(5.0);

// Import all exported items
import * from MathUtils;
square(5.0);
PI;

// Relative imports
import { User } from "./models/user.pln";
import { Database } from "../db/connection.pln";
```

### 8.3 Package Management

```pleno
// File: pleno.toml (package configuration)
{
    "name": "my-app",
    "version": "1.0.0",
    "dependencies": {
        "http-client": "^2.0.0",
        "json-parser": "^1.5.0"
    },
    "entry": "main.pln"
}

// Using external packages
import { HttpClient } from "http-client";
import { parse } from "json-parser";
```

---

## 9. Async/Concurrency ✅

### 9.1 Async/Await

```pleno
// Async function
async fn fetchData(url: String): Promise<String> {
    let response = await httpGet(url);
    return response.body;
}

// Using async function
async fn main() {
    try {
        let data = await fetchData("https://api.example.com");
        print(data);
    } catch (e: Error) {
        print("Error: " + e.message);
    }
}
```

### 9.2 Promises

```pleno
// Creating a promise
let promise = Promise<Int>(|resolve, reject| => {
    // Async work
    if (success) {
        resolve(42);
    } else {
        reject(Error("Failed"));
    }
});

// Promise chaining
promise
    .then(|value| => value * 2)
    .then(|value| => print(value))
    .catch(|error| => print(error));

// Promise.all
let results = await Promise.all([
    fetchData("url1"),
    fetchData("url2"),
    fetchData("url3")
]);

// Promise.race
let fastest = await Promise.race([
    fetchData("url1"),
    fetchData("url2")
]);
```

### 9.3 Parallel Execution

```pleno
// Spawn concurrent task
let task1 = spawn(fn() => heavyComputation1());
let task2 = spawn(fn() => heavyComputation2());

// Wait for results
let result1 = await task1;
let result2 = await task2;
```

### 9.4 Channels (Message Passing)

```pleno
// Create channel
let channel = Channel<Int>();

// Send to channel
spawn(fn() => {
    for (i in 0..10) {
        channel.send(i);
    }
    channel.close();
});

// Receive from channel
for (value in channel) {
    print(value);
}
```

---

## 10. Generics

### 10.1 Generic Functions

```pleno
fn identity<T>(value: T): T {
    return value;
}

let num = identity<Int>(42);
let str = identity<String>("hello");
let inferred = identity(3.14);  // Type inferred as Float
```

### 10.2 Generic Classes

```pleno
class Box<T> {
    private value: T;

    constructor(value: T) {
        this.value = value;
    }

    public fn get(): T {
        return this.value;
    }

    public fn set(value: T): Void {
        this.value = value;
    }
}

let intBox = Box<Int>(42);
let strBox = Box<String>("hello");
```

### 10.3 Generic Constraints

```pleno
// Constraint: T must implement Comparable
fn findMax<T: Comparable>(items: T[]): T? {
    if (items.isEmpty()) return None;

    let max = items[0];
    for (item in items) {
        if (item > max) {
            max = item;
        }
    }
    return Some(max);
}

// Multiple constraints
fn process<T: Serializable & Cloneable>(item: T): T {
    let copy = item.clone();
    serialize(copy);
    return copy;
}
```

### 10.4 Generic Interfaces

```pleno
interface Container<T> {
    fn add(item: T): Void;
    fn get(index: Int): T?;
    fn size(): Int;
}

class ArrayList<T> implements Container<T> {
    private items: T[] = [];

    fn add(item: T): Void {
        this.items.push(item);
    }

    fn get(index: Int): T? {
        if (index >= 0 && index < this.items.length) {
            return Some(this.items[index]);
        }
        return None;
    }

    fn size(): Int {
        return this.items.length;
    }
}
```

---

## 11. Decorators/Attributes

```pleno
// Decorator definition
@decorator
fn logged(target: Function): Function {
    return fn(...args) => {
        print("Calling function with args: " + args);
        let result = target(...args);
        print("Result: " + result);
        return result;
    };
}

// Using decorator
@logged
fn calculate(x: Int, y: Int): Int {
    return x + y;
}

// Multiple decorators
@authenticated
@cached
@logged
fn getUserData(userId: Int): User {
    // implementation
}

// Class decorators
@serializable
@observable
class User {
    public name: String;
    public age: Int;
}
```

---

## 12. Memory Management & Performance

### 12.1 Ownership Hints (Optional)

```pleno
// Move semantics (transfer ownership)
fn takeOwnership(value: move String) {
    // value is moved, original is invalidated
}

// Borrow (read-only reference)
fn borrowValue(value: &String) {
    // value is borrowed, cannot be modified
}

// Mutable borrow
fn borrowMutable(value: &mut String) {
    // value can be modified
}
```

### 12.2 Lazy Evaluation

```pleno
// Lazy evaluation
let lazy lazyValue = expensiveComputation();
// Computed only when accessed
print(lazyValue);  // Computation happens here

// Lazy sequences
let lazySeq = (0..)
    .map(|x| => x * x)
    .filter(|x| => x % 2 == 0)
    .take(10);  // Only computes 10 items
```

### 12.3 Memory Pooling

```pleno
// Object pooling
@pooled(size: 100)
class GameObject {
    // Instances recycled from pool
}
```

---

## 13. Standard Library Highlights ✅

### 13.1 Global Functions ✅

#### `print(...values, sep=" ", end="\n")` ✅

The `print` function outputs values to the standard output. It supports multiple arguments and keyword arguments for custom formatting.

- `values`: Zero or more values to print.
- `sep`: String to insert between values (default: `" "`).
- `end`: String to append after all values (default: `"\n"`).

```pleno
print("A", "B", "C");             // Output: A B C\n
print("A", "B", "C", sep=", ");   // Output: A, B, C\n
print("A", end="!"); print("B");  // Output: A!B\n
```

#### `input(prompt?: String): String` ✅

Reads a line from standard input, optionally printing a prompt message first.

```pleno
let name = input("Enter name: ");
let raw = input(); // No prompt
```

#### `assert(condition: Bool, message?: String): Void` ✅

Throws an error if the condition is false.

```pleno
assert(x > 0, "x must be positive");
```

### 13.2 String Operations

Strings in Pleno provide various methods for manipulation.

```pleno
let str = "Hello, World!";

str.length;                    // 13
str.toUpperCase();             // "HELLO, WORLD!"
str.toLowerCase();             // "hello, world!"
str.trim();                    // Remove whitespace
str.split(",");                // ["Hello", " World!"]
str.contains("World");         // true
str.startsWith("Hello");       // true
str.endsWith("!");             // true
str.replace("World", "Pleno"); // "Hello, Pleno!"
str.substring(0, 5);           // "Hello"
str.charAt(0);                 // "H"
str.indexOf("o");              // 4
str.repeat(3);                 // "Hello, World!Hello, World!Hello, World!"
str.toInt();                   // Convert to Int (e.g. "123" -> 123)
str.toFloat();                 // Convert to Float (e.g. "12.5" -> 12.5)

// String interpolation
let name = "Alice";
let age = 30;
let message = `Hello, ${name}! You are ${age} years old.`;

// Multi-line strings
let multiline = `
    This is a
    multi-line
    string
`;
```

### 13.2 Math Operations

```pleno
Math.abs(-5);           // 5
Math.ceil(4.3);         // 5
Math.floor(4.7);        // 4
Math.round(4.5);        // 5
Math.max(1, 5, 3);      // 5
Math.min(1, 5, 3);      // 1
Math.pow(2, 3);         // 8
Math.sqrt(16);          // 4
Math.random();          // Random float between 0 and 1
Math.randomInt(1, 10);  // Random int between 1 and 10
Math.PI;                // 3.141592653589793
Math.E;                 // 2.718281828459045
```

### 13.3 File I/O

```pleno
import { File, FileMode } from "std:io";

// Read entire file
let content = File.read("data.txt");

// Read lines
let lines = File.readLines("data.txt");
for (line in lines) {
    print(line);
}

// Write to file
File.write("output.txt", "Hello, World!");

// Append to file
File.append("log.txt", "New log entry\n");

// File operations
File.exists("data.txt");       // Check if exists
File.delete("temp.txt");       // Delete file
File.copy("src.txt", "dst.txt"); // Copy file
File.move("old.txt", "new.txt"); // Move/rename file

// Working with file handles
let file = File.open("data.txt", FileMode.Read);
try {
    let content = file.read();
    print(content);
} finally {
    file.close();
}

// Using with auto-close
with (file = File.open("data.txt", FileMode.Read)) {
    let content = file.read();
    print(content);
}  // Automatically closed
```

### 13.4 Directory Operations ✅

```pleno
import { Directory } from "std:io";

Directory.create("new_folder");
Directory.exists("folder");
Directory.delete("old_folder");
Directory.list_files(".");         // List only files
Directory.list_directories(".");   // List only directories

// Recursive operations
Directory.createRecursive("path/to/nested/folder");
Directory.deleteRecursive("folder_with_contents");
```

### 13.5 JSON Handling ✅

The `std:json` module provides a standardized `JSON` object for serialization.

```pleno
import { JSON } from "std:json";

// Parse JSON
let data = JSON.parse('{"name": "Alice", "age": 30}');
print(data.name);  // "Alice"

// Stringify to JSON
let obj = { name: "Bob", age: 25, active: true };
let jsonStr = JSON.stringify(obj);
// '{"name":"Bob","age":25,"active":true}'
```

### 13.6 HTTP Client ✅

The `std:http` module provides a namespaced `HTTP` object for web operations.

```pleno
import { HTTP } from "std:http";

// GET request
async fn fetchData() {
    let response = await HTTP.get("https://api.example.com/data");

    if (response.status == 200) {
        let data = JSON.parse(response.body);
        return data;
    } else {
        throw Error("Request failed: " + response.status);
    }
}

// POST request
async fn createUser(user: User) {
    let response = await HTTP.post(
        "https://api.example.com/users",
        body: JSON.stringify(user),
        headers: {
            "Content-Type": "application/json"
        }
    );

    return JSON.parse(response.body);
}

// Other methods
await HTTP.put(url, body: data);
await HTTP.delete(url);
await HTTP.patch(url, body: data);
```

### 13.7 Date & Time ✅

```pleno
import { DateTime, Duration } from "std:time";

// Current date/time
let now = DateTime.now();
print(now.getTimestamp());         // Unix timestamp

// Formatting (chrono-style format)
print(now.format("%Y-%m-%d %H:%M:%S"));

// Parsing
let parsed = DateTime.parse("2026-01-22", "%Y-%m-%d");

// Duration
let duration = Duration.hours(2);
let future = now.add(duration);
let past = now.subtract(Duration.days(7));
```

    print("date1 is later");

}

````

### 13.8 Regular Expressions

```pleno
import { Regex } from "std:regex";

let pattern = Regex(r"\d+");  // Raw string literal

// Test match
if (pattern.test("123")) {
    print("Matched!");
}

// Find matches
let matches = pattern.findAll("abc 123 def 456");
// ["123", "456"]

// Replace
let result = pattern.replace("abc 123 def", "XXX");
// "abc XXX def"

// Capture groups
let emailPattern = Regex(r"(\w+)@(\w+\.\w+)");
let match = emailPattern.match("user@example.com");
if (match) {
    print(match.group(1));  // "user"
    print(match.group(2));  // "example.com"
}
````

### 13.9 Low-level Networking

The `std:net` module provides low-level TCP networking primitives.

```pleno
import { Net } from "std:net";

// TCP Listener - creates a server socket
let listener = Net.listen("127.0.0.1:8080");

// TCP Connection - connects to a remote host
let connection = Net.connect("example.com:80");
```

#### Net API Reference

| Method    | Signature                                 | Description                                |
| --------- | ----------------------------------------- | ------------------------------------------ |
| `listen`  | `Net.listen(addr: String) -> Listener`    | Bind to address and listen for connections |
| `connect` | `Net.connect(addr: String) -> Connection` | Connect to a remote TCP endpoint           |

#### Listener Object

| Property  | Type     | Description                                |
| --------- | -------- | ------------------------------------------ |
| `address` | `String` | The bound address (e.g., "127.0.0.1:8080") |

#### Connection Object

| Property        | Type     | Description                 |
| --------------- | -------- | --------------------------- |
| `remoteAddress` | `String` | The remote endpoint address |

> [!NOTE]
> For higher-level HTTP functionality, use `std:http` instead.

### 13.10 Testing Framework (std:test) ✅

The `std:test` module provides a Jest/Mocha-style testing framework with assertions.

```pleno
import { describe, test, beforeEach, afterEach, assert } from "std:test";

describe("Math operations", fn() {
    let value = 0;

    beforeEach(fn() {
        value = 10;  // Setup before each test
    });

    afterEach(fn() {
        value = 0;   // Cleanup after each test
    });

    test("addition works", fn() {
        assert.equal(value + 5, 15);
    });

    test("subtraction works", fn() {
        assert.equal(value - 3, 7);
    });

    describe("edge cases", fn() {
        test("zero handling", fn() {
            assert.equal(value * 0, 0);
        });
    });
});
```

#### Assertions

| Method                              | Description                        |
| ----------------------------------- | ---------------------------------- |
| `assert.equal(a, b)`                | Deep equality check                |
| `assert.notEqual(a, b)`             | Deep inequality check              |
| `assert.true(value)`                | Expects value to be `true`         |
| `assert.false(value)`               | Expects value to be `false`        |
| `assert.null(value)`                | Expects value to be `null`         |
| `assert.notNull(value)`             | Expects value to not be `null`     |
| `assert.throws(fn)`                 | Expects function to throw an error |
| `assert.contains(collection, item)` | Checks if collection contains item |
| `assert.lengthOf(collection, n)`    | Checks collection length equals n  |

### 13.11 Collection API Reference ✅

#### Array Methods

```pleno
let arr = [1, 2, 3, 4, 5];

// Mutating Methods
arr.push(6);              // Add to end
arr.pop();                // Remove from end, returns removed item
arr.shift();              // Remove from start, returns removed item
arr.unshift(0);           // Add to start
arr.insert(2, 99);        // Insert at index
arr.remove(1);            // Remove at index, returns removed item
arr.clear();              // Remove all elements
arr.reverse();            // Reverse in place

// Non-mutating Methods
arr.length;               // Property: number of elements
arr.isEmpty();            // Check if empty
arr.contains(3);          // Check if contains element
arr.indexOf(3);           // Find index of element (-1 if not found)
arr.slice(1, 3);          // Get subarray [start, end)
arr.join(", ");           // Join elements with separator

// Functional Methods (return new array/value)
arr.map(|x| => x * 2);              // Transform each element
arr.filter(|x| => x > 2);           // Keep elements matching predicate
arr.reduce(|acc, x| => acc + x, 0); // Accumulate to single value
arr.find(|x| => x > 3);             // Find first matching element
arr.forEach(|x| => print(x));       // Execute for each element
arr.sort();                         // Return sorted copy
arr.with(0, 99);                    // Return copy with element at index replaced

// Iteration
arr.iterator();           // Get iterator
arr.entries();            // Get (index, value) iterator
```

#### Map Methods

```pleno
let map = { "a": 1, "b": 2 };

// Mutating Methods
map.put("c", 3);          // Add/update key-value pair
map.remove("a");          // Remove key, returns success boolean
map.clear();              // Remove all entries

// Non-mutating Methods
map.get("a");             // Get value for key (null if missing)
map.containsKey("a");     // Check if key exists
map.keys();               // Get array of keys
map.values();             // Get array of values
map.size();               // Get number of entries

// Functional Methods
map.map(|k, v| => v * 2);           // Transform values
map.filter(|k, v| => v > 1);        // Keep matching entries
map.forEach(|k, v| => print(k));    // Execute for each entry
map.with("d", 4);                   // Return copy with new entry

// Iteration
map.iterator();           // Get key iterator
map.entries();            // Get (key, value) iterator
```

#### Set Methods

```pleno
let set = Set([1, 2, 3]);

// Mutating Methods
set.add(4);               // Add element
set.remove(2);            // Remove element, returns success boolean
set.clear();              // Remove all elements

// Non-mutating Methods
set.contains(3);          // Check if contains element
set.size();               // Get number of elements
set.toArray();            // Convert to array

// Set Operations
set.union(otherSet);      // Elements in either set
set.intersection(other);  // Elements in both sets
set.difference(other);    // Elements in this but not other

// Functional Methods
set.map(|x| => x * 2);    // Transform elements
set.filter(|x| => x > 1); // Keep matching elements
set.forEach(|x| => print(x));
set.with(5);              // Return copy with element added

// Iteration
set.iterator();           // Get iterator
```

---

## 14. Advanced Features ✅

### 14.1 Reflection & Metaprogramming ✅

The `std:reflect` module provides metadata and dynamic access to objects.

```pleno
import { Reflect } from "std:reflect";

class User {
    public name: String;
    public age: Int;
}

let user = User();

// Get class info
let className = Reflect.getTypeName(user);  // "User"
let fields = Reflect.getFields(user);       // ["name", "age"]
let methods = Reflect.getMethods(user);

// Get/set field values dynamically
Reflect.setField(user, "name", "Alice");
let name = Reflect.getField(user, "name");

// Check type at runtime
if (Reflect.isInstanceOf(user, User)) {
    print("Is a User instance");
}

// Call method dynamically
Reflect.callMethod(user, "someMethod", [arg1, arg2]);
```

### 14.2 Macros (Compile-time Code Generation) ✅

```pleno
// Define macro
macro repeat(count, code) {
    for (let i = 0; i < count; i++) {
        ${code}
    }
}

// Use macro
repeat!(3, {
    print("Hello");
});

// Expands to:
// print("Hello");
// print("Hello");
// print("Hello");
```

### 14.3 Operator Overloading ✅

```pleno
class Vector {
    private x: Float;
    private y: Float;

    constructor(x: Float, y: Float) {
        this.x = x;
        this.y = y;
    }

    // Overload + operator
    operator +(other: Vector): Vector {
        return Vector(this.x + other.x, this.y + other.y);
    }

    // Overload * operator (scalar multiplication)
    operator *(scalar: Float): Vector {
        return Vector(this.x * scalar, this.y * scalar);
    }

    // Overload == operator
    operator ==(other: Vector): Bool {
        return this.x == other.x && this.y == other.y;
    }

    // Overload [] operator
    operator [](index: Int): Float {
        match (index) {
            case 0 => return this.x,
            case 1 => return this.y,
            case _ => throw Error("Index out of bounds")
        }
    }
}

let v1 = Vector(1.0, 2.0);
let v2 = Vector(3.0, 4.0);
let v3 = v1 + v2;          // Vector(4.0, 6.0)
let v4 = v1 * 2.0;         // Vector(2.0, 4.0)
print(v1 == v2);           // false
print(v1[0]);              // 1.0
```

### 14.4 Extension Methods ✅

```pleno
// Extend existing types
extend String {
    fn reversed(): String {
        return this.split("").reverse().join("");
    }

    fn isPalindrome(): Bool {
        return this == this.reversed();
    }
}

// Usage
print("hello".reversed());      // "olleh"
print("racecar".isPalindrome()); // true

// Extend generic types
extend <T> Array<T> {
    fn second(): T? {
        if (this.length > 1) {
            return Some(this[1]);
        }
        return None;
    }
}

let numbers = [1, 2, 3];
print(numbers.second());  // Some(2)
```

### 14.5 Annotations & Metadata ✅

```pleno
// Define custom annotation
@annotation
class Route {
    public path: String;
    public method: String;

    constructor(path: String, method: String = "GET") {
        this.path = path;
        this.method = method;
    }
}

// Use annotation
@Route("/users", "GET")
fn getUsers(): User[] {
    // implementation
}

@Route("/users", "POST")
fn createUser(user: User): User {
    // implementation
}

// Read annotations at runtime
let annotations = Reflect.getAnnotations(getUsers);
```

### 14.6 Enum Types ✅

```pleno
// Simple enum
enum Color {
    Red,
    Green,
    Blue
}

let color = Color.Red;

// Enum with values
enum Status {
    Pending = 0,
    Active = 1,
    Completed = 2
}

// Enum with associated data
enum Result<T, E> {
    Ok(T),
    Err(E)
}

let success: Result<Int, String> = Result.Ok(42);
let failure: Result<Int, String> = Result.Err("Failed");

// Pattern matching on enums
match (success) {
    case Result.Ok(value) => print("Success: " + value),
    case Result.Err(error) => print("Error: " + error)
}

// Enum methods
enum Direction {
    North,
    South,
    East,
    West;

    fn opposite(): Direction {
        match (this) {
            case Direction.North => return Direction.South,
            case Direction.South => return Direction.North,
            case Direction.East => return Direction.West,
            case Direction.West => return Direction.East
        }
    }
}
```

### 14.7 Type Aliases ✅

```pleno
// Simple type alias
type UserId = Int;
type UserName = String;

// Generic type alias
type Result<T> = Result<T, Error>;
type HashMap<K, V> = Map<K, V>;

// Complex type alias
type Handler = (Request, Response) => Void;
type Predicate<T> = (T) => Bool;

// Using type aliases
fn processUser(id: UserId, name: UserName): Void {
    // implementation
}

let handler: Handler = fn(req, res) => {
    res.send("Hello");
};
```

### 14.8 Union & Intersection Types ✅

```pleno
// Union type
type StringOrNumber = String | Int | Float;

fn process(value: StringOrNumber) {
    if (value is String) {
        print(value.toUpperCase());
    } else {
        print(value * 2);
    }
}

// Intersection type (must satisfy all constraints)
interface Nameable {
    fn getName(): String;
}

interface Ageable {
    fn getAge(): Int;
}

type Person = Nameable & Ageable;

// Function accepting intersection type
fn displayPerson(p: Person) {
    print(p.getName() + " is " + p.getAge() + " years old");
}
```

---

## 15. Testing Framework ✅

```pleno
import { test, assert, describe, beforeEach, afterEach } from "std:test";

// Test suite
describe("Calculator", fn() {
    let calc: Calculator;

    beforeEach(fn() {
        calc = Calculator();
    });

    afterEach(fn() {
        calc = null;
    });

    test("should add numbers correctly", fn() {
        let result = calc.add(2, 3);
        assert.equal(result, 5);
    });

    test("should handle negative numbers", fn() {
        let result = calc.add(-5, 3);
        assert.equal(result, -2);
    });

    test("should throw on division by zero", fn() {
        assert.throws(fn() => calc.divide(10, 0));
    });
});

// Assertions
assert.equal(actual, expected);
assert.notEqual(actual, expected);
assert.true(value);
assert.false(value);
assert.null(value);
assert.notNull(value);
assert.throws(fn);
assert.contains(collection, item);
assert.lengthOf(collection, length);
```

---

## 16. Package Manager (PLM - Pleno Module Manager)

```bash
# Initialize new project
plm init

# Install package
plm install http-client

# Install specific version
plm install json-parser@1.5.0

# Install dev dependency
plm install --dev test-framework

# Update packages
plm update

# Remove package
plm remove http-client

# Run script
plm run build
plm run test
```

```toml
# pleno.toml
[package]
name = "my-app"
version = "1.0.0"
description = "My Pleno application"
main = "src/main.pln"

[scripts]
start = "pleno run src/main.pln"
build = "pleno build --output dist/"
test = "pleno test tests/"

[dependencies]
http-client = "^2.1.0"
json-parser = "^1.5.0"

[dev-dependencies]
test-framework = "^3.0.0"
```

---

---

## 17. Example Programs

### 17.1 Hello World

```pleno
fn main() {
    print("Hello, World!");
}

main();
```

### 17.2 Web Server

```pleno
import { Server, Router } from "std:http";

let router = Router();

router.get("/", fn(req, res) {
    res.send("Hello, Pleno!");
});

router.get("/users/:id", fn(req, res) {
    let userId = req.params.id;
    res.json({ id: userId, name: "User " + userId });
});

router.post("/users", fn(req, res) {
    let user = JSON.parse(req.body);
    // Save user to database
    res.status(201).json(user);
});

let server = Server(router);
server.listen(8080, fn() {
    print("Server running on port 8080");
});
```

### 17.3 Todo CLI Application

```pleno
import { File } from "std:io";
import { JSON } from "std:json";

class TodoManager {
    private todos: Array<Todo> = [];
    private file: String = "todos.json";

    constructor() {
        this.load();
    }

    fn add(title: String) {
        let todo = Todo(title);
        this.todos.push(todo);
        this.save();
        print("Added: " + title);
    }

    fn list() {
        if (this.todos.isEmpty()) {
            print("No todos yet!");
            return;
        }

        for (index, todo in this.todos) {
            let status = todo.completed ? "[✓]" : "[ ]";
            print(`${index + 1}. ${status} ${todo.title}`);
        }
    }

    fn complete(index: Int) {
        if (index >= 0 && index < this.todos.length) {
            this.todos[index].completed = true;
            this.save();
            print("Completed: " + this.todos[index].title);
        }
    }

    fn remove(index: Int) {
        if (index >= 0 && index < this.todos.length) {
            let removed = this.todos.remove(index);
            this.save();
            print("Removed: " + removed.title);
        }
    }

    private fn save() {
        File.write(this.file, JSON.stringify(this.todos));
    }

    private fn load() {
        if (File.exists(this.file)) {
            let content = File.read(this.file);
            this.todos = JSON.parse(content);
        }
    }
}

class Todo {
    public title: String;
    public completed: Bool = false;

    constructor(title: String) {
        this.title = title;
    }
}

fn main(args: String[]) {
    let manager = TodoManager();

    if (args.length == 0) {
        print("Usage: todo [add|list|complete|remove] ...");
        return;
    }

    match (args[0]) {
        case "add" => {
            if (args.length > 1) {
                manager.add(args[1..].join(" "));
            }
        },
        case "list" => manager.list(),
        case "complete" => {
            if (args.length > 1) {
                manager.complete(Int.parse(args[1]) - 1);
            }
        },
        case "remove" => {
            if (args.length > 1) {
                manager.remove(Int.parse(args[1]) - 1);
            }
        },
        case _ => print("Unknown command")
    }
}
```

---

## 18. Performance Features ✅

_Note: Advanced performance features like JIT and advanced GC hints are planned for future releases. Phase 1 optimizations (DCE, Inlining) are implemented._

### 18.1 JIT Compilation

- Hot path optimization
- Automatic inlining of small functions
- Dead code elimination

### 18.2 Memory Management

- Generational garbage collection
- Reference counting for immediate cleanup
- Optional manual memory hints
- Object pooling support

### 18.3 Optimization Hints

```pleno
// Inline hint
@inline
fn fastOperation(x: Int): Int {
    return x * 2;
}

// No optimization (for debugging)
@no_optimize
fn debugFunction() {
    // Keep unoptimized for debugging
}

// Hot path hint
@hot
fn criticalPath() {
    // Prioritize optimization
}
```

---

## 19. Developer Experience & Diagnostics ✅

Pleno is designed with a focus on helpful error messages and developer tooling. The diagnostic system provides clear, "Rust-like" error reporting with source-level spans, multi-label diagnostics, and actionable hints.

### 19.1 Advanced Diagnostics

```
Error: Expected ';' after let statement
   --> file:10:5
    |
 10 |     let x = 10
    |                ^ expected ';' here
```

The diagnostic system supports:

- **Visual Spans**: Precisely underlining problematic code.
- **Multiple Labels**: One error can point to multiple related locations in the source.
- **Hints and Notes**: Providing suggestions for how to fix the error.
- **Colorized Output**: High-contrast, color-coded output for different severity levels.

---

## 20. Security Features ✅

_Note: Sandboxing, resource limits, and hardened capability-based IO are implemented._

### 20.1 Safe Defaults

- No implicit type coercion
- Strict null checking
- Bounds checking on arrays
- Integer overflow detection

### 20.2 Sandboxing

```pleno
import { Sandbox } from "std:security";

// Run untrusted code in sandbox
let sandbox = Sandbox({
    allowIO: true,
    allowedReadPaths: ["/tmp/data"],
    allowNetwork: false,
    maxMemory: 10 * 1024 * 1024  // 10 MB
});

try {
    let result = sandbox.run(fn() {
        // Untrusted code here
        return calculate(data);
    });
} catch (e) {
    print("Sandbox violation: " + e.message);
}
```

---

## 21. Conclusion

Pleno is designed to be:

- **Easy to learn**: Familiar syntax, clear semantics
- **Safe by default**: Strong typing, real encapsulation, memory safety
- **Powerful**: OOP, functional, and procedural paradigms
- **Modern**: Async/await, pattern matching, generics
- **Practical**: Great standard library, package manager, tooling

This specification provides a complete foundation for a modern programming language suitable for 2026 and beyond.
