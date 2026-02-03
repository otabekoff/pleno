# Getting Started with Pleno

Welcome to Pleno! This guide will help you install Pleno and write your first program.

## Installation

### Prerequisites

- **Rust**: Pleno is built with Rust. Ensure you have Rust installed (`rustup`).
- **Git**: For cloning the repository.

### Installing from Source

```bash
git clone https://github.com/otabekoff/pleno.git
cd pleno
cargo build --release
```

Add the binary to your path:

```bash
# Linux/macOS
export PATH="$PWD/target/release:$PATH"

# Windows (PowerShell)
$env:PATH += ";$PWD\target\release"
```

### Verify Installation

```bash
pleno --version
# Pleno 0.1.0
```

## Your First Program

Create a file named `hello.pln`:

```pleno
fn main() {
    print("Hello, Pleno!");
}

main();
```

Run it:

```bash
pleno run hello.pln
```

## Language Basics

### Variables

```pleno
let name = "Alice";        // Immutable by default
let mut counter = 0;       // Mutable variable
const PI = 3.14159;        // Constant
```

### Functions

```pleno
fn greet(name: String): String {
    return "Hello, " + name + "!";
}

print(greet("World"));  // Hello, World!
```

### Control Flow

```pleno
let score = 85;

if (score >= 90) {
    print("A");
} else if (score >= 80) {
    print("B");
} else {
    print("C");
}

// For loop
for (i in 0..5) {
    print(i);
}
```

### Classes

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

let alice = Person("Alice", 30);
print(alice.greet());
```

### Async/Await

```pleno
async fn fetchData(url: String) {
    let response = await HTTP.get(url);
    return response.body;
}

async fn main() {
    let data = await fetchData("https://api.example.com");
    print(data);
}
```

## CLI Commands

```bash
pleno run script.pln      # Run a script
pleno repl                 # Start interactive REPL
pleno check script.pln     # Type check without running
pleno format src/          # Format code
pleno test tests/          # Run tests
```

## Next Steps

- Explore the [Language Specification](/specs/)
- Browse the [Standard Library](/stdlib/)
- Check out the [Examples](/examples)
- Learn about [Contributing](contributing.md)
