# Getting Started with Pleno

Welcome to Pleno! This guide will help you install Pleno and write your first program.

## Installation

### Prerequisites

- **Rust**: Pleno is built with Rust. Ensure you have Rust installed.
- **Git**: For cloning the repository.

### Installing from Source

```bash
git clone https://github.com/otabekoff/pleno.git
cd pleno
cargo build --release
```

Add the binary to your path (optional but recommended):
```bash
# Windows (PowerShell)
$env:PATH += ";$PWD\target\release"
```

## Your First Program

Create a file named `hello.pln`:

```pleno
fn main() {
    print("Hello, Pleno!");
}

main();
```

Run it with the Pleno CLI:

```bash
cargo run --bin pleno-cli -- run hello.pln
# Or if installed:
pleno run hello.pln
```

## Next Steps

- Explore the [Language Specification](../specs/spec.md)
- Learn about [Contributing](../guide/contributing.md)
- Check out the [Examples](https://github.com/otabekoff/pleno/tree/main/examples)
