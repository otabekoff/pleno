# Getting started

This page assumes Pleno is [installed](./installation).

## Your first program

Create `hello.pln`:

```pleno
fn greet(name: String): String {
    return `Hello, ${name}!`;
}

print(greet("Pleno"));
```

Run it:

```sh
pleno run hello.pln
```

`pleno check hello.pln` finds syntax errors, misspelled names, calls with the wrong number of arguments and literals that do not match their type annotation, without running the file.

## A project

```sh
mkdir my-app
cd my-app
plm init my-app
plm run dev    # runs src/main.pln
plm test       # runs the tests in tests/
```

`plm init` creates `pleno.toml` (the name, scripts and dependencies), `src/main.pln`, a test in `tests/`, and editor settings. See [Packages](./packages) for dependencies.

## The language in a few minutes

### Values and variables

```pleno
let name = "Ada";          // cannot be reassigned
let mut count = 0;         // can be
const LIMIT = 10;
count += 1;

let numbers = [1, 2, 3];
let ages = {"Ada": 36, "Alan": 41};
let point = (x: 1.5, y: 2.0);   // a named tuple
print(numbers.length, ages["Ada"], point.x);
```

### Types, where you want them

```pleno
fn area(width: Float, height: Float): Float {
    return width * height;
}

let maybe: Int? = null;      // Int or null
let id: Int | String = 42;   // either
```

A value of the wrong type raises an error when the code runs: `area("3", 4.0)` fails with `parameter 'width' expected Float, got String`.

### Control flow

```pleno
let score = 85;
if (score >= 90) {
    print("A");
} else if (score >= 80) {
    print("B");
} else {
    print("C");
}

for (i in 0..3) {
    print(i);   // 0, 1, 2
}

let grade = "B";
let label = match (grade) {
    case "A" => "excellent",
    case "B" => "good",
    case _ => "keep going"
};
print(label);   // good
```

### Classes and enums

```pleno
class Person {
    public name: String;
    private age: Int;

    constructor(name: String, age: Int) {
        this.name = name;
        this.age = age;
    }

    fn greet(): String {
        return "Hi, I'm " + this.name;
    }
}

enum Shape {
    Circle(radius),
    Square(side)
}

fn area(shape: Shape): Float {
    match (shape) {
        case Shape.Circle(r) => return 3.14159 * r * r,
        case Shape.Square(s) => return s * s
    }
}

print(Person("Ada", 36).greet(), area(Shape.Square(2.0)));
```

### Errors

```pleno
fn parsePort(text: String): Int {
    let port = text.toInt();
    if (port < 1 || port > 65535) {
        throw Error(`invalid port ${text}`);
    }
    return port;
}

try {
    parsePort("70000");
} catch (e) {
    print(e.message);
}
```

### Async code

```pleno
import { HTTP } from "std:http";

async fn fetchTitle(url: String): String {
    let response = await HTTP.get(url);
    return response.body.substring(0, 60);
}

let task = spawn(fn() => 6 * 7);
print(await task);   // 42
```

## Next

- [Set up your editor](./editor)
- [Write tests](./testing)
- Read the [language reference](/specs/) and the [standard library](/stdlib/)
