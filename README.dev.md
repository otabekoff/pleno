# Pleno Developer Guide

Welcome to the Pleno internal development guide. This document is intended for maintainers and contributors working on the core language implementation.

## 📖 Navigation

- **[Architecture Deep Dive](ARCHITECTURE.md)**: Detailed technical overview of VM and Compiler internals.
- **[Language Specification](specs.md)**: The authoritative reference for Pleno syntax and semantics.
- **[Contributing Guidelines](CONTRIBUTING.md)**: How to submit your changes.

---

## 🚀 Getting Started

### Prerequisites

- **Rust 1.75+**: We use modern features like `let_chains` where available (via feature flags).
- **Make**: Standard build tool.
- **Python 3**: Used for infrastructure scripts and benchmarks.

### Local Setup

```bash
# Clone the repo
git clone https://github.com/otabekoff/pleno.git
cd pleno

# Initial setup (installs hooks and tools)
make setup

# Build everything
make build
```

---

## 🏗️ Project Breakdown

Pleno is a workspace of modular crates:

| Crate               | Purpose                                       |
| ------------------- | --------------------------------------------- |
| `pleno-cli`         | The entry point (`pleno` command).            |
| `pleno-ast`         | Shared AST nodes and source span utilities.   |
| `pleno-lexer`       | Lexical scanner (regex-free for performance). |
| `pleno-parser`      | Syntax analyzer (Recursive Descent).          |
| `pleno-compiler`    | Bytecode generation and basic optimizations.  |
| `pleno-bytecode`    | Instruction definitions and binary format.    |
| `pleno-vm`          | Core execution engine (Register-based).       |
| `pleno-value`       | Core data types and object system internals.  |
| `pleno-stdlib`      | Standard library implemented in Rust.         |
| `pleno-diagnostics` | Pretty-printing errors and spans.             |

---

## 🛠️ Development Workflow

### Useful Make Targets

The `Makefile` in the root is the preferred way to run common tasks:

```bash
make build          # Build in debug mode
make release        # Build in release mode (for benchmarks)
make test           # Run all unit and integration tests
make lint           # Run clippy and format checks
make check          # Full hygiene check (format + lint + test)
make clean          # Remove build artifacts and logs
```

### Compiler/VM Debugging

Developing a language requires deep visibility into internal state. Use these flags:

#### 1. Trace Instruction Execution

```bash
# See every instruction the VM executes
RUST_LOG=pleno_vm=trace pleno run my_script.pln
```

#### 2. Inspect Bytecode

```bash
# Compile to human-readable bytecode dump
pleno compile --dump-bytecode my_script.pln
```

#### 3. AST Inspection

```bash
# Print the AST as a JSON or pretty-formatted tree
pleno debug-ast my_script.pln
```

---

## 🧪 Advanced Testing

### Source Tests

Most language features should be tested using "Source Tests" in `tests/sources_tests/`. These are `.pln` files that the VM runs, comparing the output against a `.expected` file or internal assertions.

### Benchmark Regression

Before major VM changes, run the regression suite:

```bash
python3 bench/run_bench.py --baseline
# ... make changes ...
python3 bench/run_bench.py --compare
```

---

## 💡 Troubleshooting

### "Unterminated string literal at start of file"

This usually indicates a Lexer failure. Check if there are invisible BOM characters or encoding issues in the test file.

### "Instruction pointer out of bounds"

The compiler generated a jump to an invalid index. This is a compiler bug. Use `pleno compile --dump-bytecode` to find the invalid jump target.

### "Stack Overflow (Recursive Calls)"

Pleno VM has a default recursion limit of 1000 frames. If you hit this legitimately, you might need to use `loop` or increase the limit in `VMConfig`.

---

## 📦 Release Procedure

We use Semantic Versioning (SemVer).

1. Update version in root `Cargo.toml`.
2. Run `make check` to ensure perfection.
3. Update `CHANGELOG.md`.
4. Create a signed tag: `git tag -s v0.x.x -m "Release v0.x.x"`.
5. Push: `git push origin main --tags`.

The CI pipeline will automatically build binaries for Linux, macOS (Intel/M1), and Windows.

---

**Happy Hacking! 🛠️**
