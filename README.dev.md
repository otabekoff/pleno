# Pleno Developer Guide

Welcome to the Pleno programming language development guide! This document provides everything you need to build, test, and contribute to Pleno.

## Table of Contents

- [Getting Started](#getting-started)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Code Quality](#code-quality)
- [Architecture Overview](#architecture-overview)
- [Contributing](#contributing)

---

## Getting Started

### Prerequisites

- **Rust** 1.70+ (install via [rustup](https://rustup.rs/))
- **Git** for version control
- **Node.js** 18+ (for documentation site)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/otabekoff/pleno.git
cd pleno

# Build the project
cargo build

# Build optimized release version
cargo build --release

# Run the REPL
cargo run --bin pleno-cli -- repl

# Run a Pleno file
cargo run --bin pleno-cli -- run examples/hello_world.pln
```

---

## Project Structure

```
pleno/
├── crates/              # Rust workspace crates
│   ├── pleno-cli/       # Command-line interface
│   ├── pleno-lexer/     # Lexical analyzer
│   ├── pleno-parser/    # Syntax parser
│   ├── pleno-compiler/  # Bytecode compiler
│   ├── pleno-vm/        # Virtual machine
│   ├── pleno-stdlib/    # Standard library
│   ├── pleno-package/   # Package manager
│   ├── pleno-doc/       # Documentation generator
│   └── ...              # Other crates
├── examples/            # Example Pleno programs
├── tests/               # Integration tests
│   └── fixtures/        # Test data (mock HTTP, JSON, etc.)
├── dev-tests/           # Development/debugging tests
│   ├── async/           # Async feature tests
│   ├── stdlib/          # Standard library tests
│   ├── features/        # Language feature tests
│   └── ...
├── sources/             # Additional source files
├── docs/                # Documentation site
├── scripts/             # Utility scripts
├── tasks/               # Project management
└── .logs/               # Log files (gitignored)
```

---

## Development Workflow

### Common Commands

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p pleno-vm
cargo test -p pleno-parser

# Run with output (for debugging)
cargo test -p pleno-vm --test vm_exec -- --nocapture

# Run formatter
cargo fmt --all

# Run linter (strict mode)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Build documentation
cargo doc --workspace --no-deps --open
```

### Advanced CLI Tools

Pleno provides several tools for language developers to inspect internal state:

```bash
# Debug AST (pretty-prints the Abstract Syntax Tree)
cargo run --bin pleno-cli -- debug-ast path/to/file.pln

# Compile to bytecode (.plc file)
cargo run --bin pleno-cli -- compile path/to/file.pln

# Watch mode (automatically re-runs file on changes)
cargo run --bin pleno-cli -- watch path/to/file.pln

# Type Check/Semantic Check (without full execution)
cargo run --bin pleno-cli -- check path/to/file.pln

# Start Language Server (LSP)
cargo run --bin pleno-cli -- lsp
```

### Enhanced Diagnostics

Pleno provides improved error reporting with actionable hints:
- **Parser Hints**: Suggests fixes for common syntax errors (e.g., missing semicolons).
- **Diagnostics**: Labeled spans point directly to the source of errors.

### Package Manager (plm)

```bash
# Initialize a new package
cargo run --bin pleno-cli -- plm init my-package

# Install dependencies
cargo run --bin pleno-cli -- plm install

# Run a script
cargo run --bin pleno-cli -- plm run dev

# Build package
cargo run --bin pleno-cli -- plm build

# Publish package
cargo run --bin pleno-cli -- plm publish
```

---

## Benchmarking

Pleno is designed for high performance. We maintain a benchmarking suite in the `bench/` directory to track performance vs Python and Node.js.

### Running Benchmarks

> [!IMPORTANT]
> Always run benchmarks using the **release** build for accurate results.

1. **Build Pleno in release mode:**
   ```bash
   cargo build --release
   ```

2. **Run the benchmark script:**
   ```bash
   # Requires Python 3 to run the orchestrator
   python3 bench/run_bench.py
   ```

### Benchmark Cases
Located in `bench/cases/`:
- `fib.pln`: Recursive Fibonacci (CPU-intensive test of Function calls and Integer ops)
- `loop.pln`: Million iterations loop (test of VM dispatch/loop overhead)

### Profiling
To identify bottlenecks, use `cargo-flamegraph` (requires `perf` on Linux):
```bash
cargo flamegraph --bin pleno-cli -- run path/to/benchmark.pln
```

---

## Testing

### Test Organization

- **Unit tests**: Located in `src/` files within each crate
- **Integration tests**: Located in `tests/` directory
- **Example tests**: Located in `examples/` directory
- **Dev tests**: Located in `dev-tests/` for debugging

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific test file
cargo test -p pleno-vm --test vm_exec

# Specific test function
cargo test -p pleno-parser test_class_parsing

# With backtrace for debugging
RUST_BACKTRACE=1 cargo test -p pleno-vm

# Show test output
cargo test -- --nocapture
```

### Writing Tests

```rust
#[test]
fn test_example() {
    let source = "let x = 42;";
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = parser.parse_program().unwrap();
    assert_eq!(program.statements.len(), 1);
}
```

---

## Code Quality

### Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting without modifying
cargo fmt --all -- --check
```

### Linting

```bash
# Run clippy with strict warnings
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Fix clippy warnings automatically (when possible)
cargo clippy --workspace --all-targets --all-features --fix
```

### Pre-commit Hooks

The project uses Husky for pre-commit hooks:
- Runs `cargo fmt` on staged Rust files
- Runs `cargo clippy` to catch issues early

---

## Architecture Overview

### Compilation Pipeline

```
Source Code (.pln)
    ↓
Lexer (pleno-lexer)
    ↓
Tokens
    ↓
Parser (pleno-parser)
    ↓
AST (pleno-ast)
    ↓
Compiler (pleno-compiler)
    ↓
Bytecode (pleno-bytecode)
    ↓
VM (pleno-vm)
    ↓
Execution
```

### Key Components

1. **Lexer**: Tokenizes source code into a stream of tokens
2. **Parser**: Builds an Abstract Syntax Tree (AST) from tokens
3. **Compiler**: Transforms AST into bytecode instructions
4. **VM**: Executes bytecode with stack-based architecture
5. **GC**: Garbage collector for memory management
6. **Standard Library**: Built-in modules (io, http, json, etc.)

### Crate Dependencies

```
pleno-cli
  ├── pleno-lexer
  ├── pleno-parser
  │     └── pleno-ast
  ├── pleno-compiler
  │     ├── pleno-ast
  │     └── pleno-bytecode
  ├── pleno-vm
  │     ├── pleno-bytecode
  │     ├── pleno-value
  │     └── pleno-gc
  ├── pleno-stdlib
  ├── pleno-package
  ├── pleno-formatter
  ├── pleno-linter
  └── pleno-doc
```

---

## Contributing

### Code Style

- Follow Rust standard conventions
- Use `cargo fmt` before committing
- Ensure `cargo clippy` passes with no warnings
- Write tests for new features
- Document public APIs with doc comments

### Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests: `cargo test --workspace`
5. Run linter: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
6. Format code: `cargo fmt --all`
7. Commit changes (`git commit -m 'Add amazing feature'`)
8. Push to branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

### Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new feature
fix: fix bug
docs: update documentation
style: format code
refactor: refactor code
test: add tests
chore: update dependencies
```

### Release Process

1. Update version in `Cargo.toml` files
2. Update `CHANGELOG.md`
3. Create a git tag: `git tag v0.x.0`
4. Push tag: `git push origin v0.x.0`
5. GitHub Actions will build and publish release

---

## Debugging Tips

### Enable Logging

```bash
# Set log level
RUST_LOG=debug cargo run --bin pleno-cli -- run test.pln

# VM-specific logging
RUST_LOG=pleno_vm=trace cargo run --bin pleno-cli -- run test.pln
```

### Debug AST

```bash
# Print AST for a file
cargo run --bin pleno-cli -- debug-ast test.pln
```

### Debug Bytecode

```rust
// In compiler or VM code
println!("{:?}", bytecode);
```

### Common Issues

**Parser errors**: Check lexer output first
**VM errors**: Enable RUST_BACKTRACE=1 for stack traces
**Memory issues**: Run with `cargo run --bin pleno-cli -- run test.pln` (debug build has better error messages)

---

## Resources

- **Specs**: See `specs.md` for language specification
- **Roadmap**: See `tasks/roadmap.md` for development roadmap
- **Contributing**: See `CONTRIBUTING.md` for contribution guidelines
- **Code of Conduct**: See `CODE_OF_CONDUCT.md`

---

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/otabekoff/pleno/issues)
- **Discussions**: [GitHub Discussions](https://github.com/otabekoff/pleno/discussions)
- **Discord**: [Join our Discord](https://discord.gg/pleno) (if available)

---

**Happy coding! 🚀**