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

##### 🛠️ Troubleshooting & Fixes

#### CI Failures

If CI fails on a PR:

1. **Check the logs**: Click on the "Details" link of the failed check.
2. **Reproduce locally**: Run `make check` locally. Most CI failures are due to formatting or clippy warnings which `make check` catches.
3. **Rust Version**: Ensure you are using the version specified in `rust-toolchain.toml` (1.75+).

#### Common Issues

- **Parser errors**: Check lexer output first using `pleno debug-ast --lexer-only`.
- **VM errors**: Enable `RUST_BACKTRACE=1` for detailed Rust-level traces.
- **Flaky Tests**: If a test fails intermittently (especially concurrency tests), it might be a race condition. Use `cargo test -- --worker-threads=1` to see if it stabilizes.

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

## 🛠️ Development Workflow & Standards

### 🌿 Branch Management

We maintain a strict branch hierarchy to ensure `main` remains production-ready:

- **`main`**: The "Source of Truth". Always stable. Direct commits are forbidden. Releases are tagged from this branch.
- **`dev`**: The integration branch. All feature branches and fixes merge here first.
- **`feature/*` / `fix/*`**: Short-lived branches for specific tasks.

**Workflow example:**

1. Checkout `dev`: `git checkout dev && git pull`
2. Create feature branch: `git checkout -b feature/new-cool-thing`
3. Work, commit, push.
4. Open PR to merge into `dev`.
5. Periodically, `dev` is merged into `main` for release.

### 📝 Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**Common Types:**

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation only changes
- `perf`: Performance improvements
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `test`: Adding missing tests
- `chore`: Build process or auxiliary tool changes

### 🏷️ Tag Management (Re-releasing)

To retag a version (e.g. `v0.1.0`) if a release failed:

1. **Delete the local and remote tag:**
   ```bash
   git tag -d v0.1.0
   git push origin :refs/tags/v0.1.0
   ```
2. **Commit fixes to `main` and push.**
3. **Re-tag and push:**
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

---

## 📦 Release Procedure

We use **Semantic Versioning (SemVer) 2.0.0**.

1. Update version in root `Cargo.toml`.
2. Run `make check` to ensure perfection.
3. Update `CHANGELOG.md`.
4. Create a signed tag: `git tag -s v0.x.x -m "Release v0.x.x"`.
5. Push to `main`: `git push origin main --tags`.

The CI pipeline will automatically build binaries for Linux, macOS, and Windows.

---

## 💡 Troubleshooting

### General Build/CI Issues

- **Clippy/Format Warnings**: Run `make check` locally. CI is set to fail on any warning.
- **VM Panics**: Enable `RUST_BACKTRACE=1`. Check for `ip` (instruction pointer) out of bounds using `pleno compile --dump-bytecode`.
- **Parsing Errors**: Use `pleno debug-ast --lexer-only` to verify tokens before investigating the parser.

### Specific Error Messages

- **"Unterminated string literal"**: Check for invisible BOM characters or encoding issues in `.pln` files.
- **"Stack Overflow"**: Pleno has a 1000-frame recursion limit. Optimize to use loops or increase `VMConfig.max_frames`.
- **"Instruction pointer out of bounds"**: Usually a compiler bug in jump target calculation. Check `Opcode::Jump` and `Opcode::JumpIfFalse`.

---

**Happy Hacking! 🛠️**
