# Advanced Features

## Reflection & Metaprogramming

```pleno
import { Reflect } from "std:reflect";

class User {
    public name: String = "";
    public age: Int = 0;

    fn greet(greeting: String): String {
        return greeting + ", " + this.name;
    }
}

let user = User();

// Get class info
print(Reflect.getTypeName(user));  // User
print(Reflect.getFields(user));    // [age, name]
print(Reflect.getMethods(user));   // [constructor, greet]

// Get/set field values dynamically
Reflect.setField(user, "name", "Alice");
print(Reflect.getField(user, "name"));  // Alice

// Check type at runtime
if (Reflect.isInstanceOf(user, User)) {
    print("Is a User instance");
}

// Call method dynamically
print(Reflect.callMethod(user, "greet", ["Hello"]));  // Hello, Alice
```

## Macros (Compile-time Code Generation)

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

## Operator Overloading

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
print(v3[0], v3[1]);       // 4 6
print(v1 == v2);           // false
print(v1[0]);              // 1
```

## Extension Methods

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
print("hello".reversed());      // olleh
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
print(numbers.second().unwrap());  // 2
```

## Annotations & Metadata

Annotations attach a name and constant arguments to a class or enum. An annotation does not need a definition. `Reflect.getAnnotations` returns them as maps with `name` and `args`; for an instance it returns the annotations of its class.

```pleno
import { Reflect } from "std:reflect";

@Route("/users", "GET")
@Version(2)
class UserController {
    fn list(): Array {
        return [];
    }
}

let annotations = Reflect.getAnnotations(UserController);
for (a in annotations) {
    print(a.name, a.args);
}
// Route [/users, GET]
// Version [2]

print(Reflect.getAnnotations(UserController()).length);  // 2
```

## Enum Types

```pleno
// Simple enum
enum Color {
    Red,
    Green,
    Blue
}

let color = Color.Red;
print(color);  // Color.Red

// Enum with values
enum Status {
    Pending = 0,
    Active = 1,
    Completed = 2
}
print(Status.Active.value);  // 1

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

print(Direction.North.opposite());  // Direction.South
```

## Type Aliases

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

processUser(1, "Ann");
```

Aliases are checked at runtime like the types they name. `let x: UserId = "oops";` throws `Type error: expected UserId, found String`.

## Union & Intersection Types

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

process("hi");  // HI
process(21);    // 42
// process(true) throws: parameter 'value' expected StringOrNumber, got Bool

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

class Employee implements Nameable, Ageable {
    fn getName(): String { return "Ann"; }
    fn getAge(): Int { return 40; }
}

let ann = Employee();
print(ann.getName() + " is " + ann.getAge() + " years old");  // Ann is 40 years old
```

> **Note**: The runtime check for an intersection type currently rejects class instances that implement all of its interfaces. `displayPerson(Employee())` throws `Type error: parameter 'p' expected Person, got Employee`.