# Contributing to Pleno

First of all, thank you for showing interest in contributing to Pleno! We want to make contributing to this project as easy and transparent as possible.

## 🙏 Ways to Contribute

- **Code**: Fix bugs, implement new features, or optimize existing ones.
- **Docs**: Improve specification, READMEs, or add tutorials.
- **Tests**: Add integration tests for language features.
- **Bug Reports**: Open issues with reproducible examples.
- **Feedback**: Suggest new features or syntax improvements.

## 🛠️ Development Environment

### Prerequisites

- **Rust**: Latest stable version (via [rustup](https://rustup.rs/)).
- **Make**: For running convenience commands.
- **Node.js**: (Optional) For VS Code extension development.

### Setup

```bash
git clone https://github.com/otabekoff/pleno.git
cd pleno
make setup
```

## 🧪 Testing Guidelines

We value correctness above all. Every new feature must be accompanied by tests.

- **Unit Tests**: Place them in the `src/` file where the logic lives.
- **Integration Tests**: Place them in `tests/`. For VM execution tests, use `crates/pleno-vm/tests/vm_exec.rs`.
- **Pleno Source Tests**: We have a large collection of `.pln` files in `tests/sources_tests/` that are executed by the VM and checked for expected output.

To run all tests:

```bash
make test
```

## 📝 Coding Standards

- **Formatting**: Always run `make format` before committing.
- **Linting**: We enforce strict linting. Run `make lint` to ensure no warnings or errors exist.
- **Idiomatic Rust**: Prefer idiomatic Rust. Use `clippy` suggestions.
- **Safety**: Avoid `unsafe` unless absolutely necessary and documented.
- **Public API**: Minimize the public surface area of crates. Use `pub(crate)` for internal logic.

## 🚀 Pull Request Process

1. **Fork** the repository and create your branch from `main`.
2. **Implement** your changes.
3. **Verify** with `make check` (this runs format, lint, and all tests).
4. **Commit** using [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat:` for new features
   - `fix:` for bug fixes
   - `docs:` for documentation changes
   - `refactor:` for code restructuring
5. **Push** and open a Pull Request.
6. **Respond** to review feedback promptly.

## ⚖️ License

By contributing to Pleno, you agree that your contributions will be licensed under the project's **MIT License**.

---

**Happy contributing! 🚀**
