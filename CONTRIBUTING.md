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

### Documentation site

The site in `docs/` uses VitePress and [Bun](https://bun.sh). Its playground and tutorial run Pleno compiled to WebAssembly, which you build once (and again after changing the Rust code):

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.108   # the version in Cargo.lock
bun install
bun run docs:wasm
bun run docs:dev
```

## 🧪 Testing Guidelines

We value correctness above all. Every new feature must be accompanied by tests.

- **Unit Tests**: Place them in the `src/` file where the logic lives.
- **Integration Tests**: Place them in the crate's `tests/` folder. For VM execution tests, use `crates/pleno-vm/tests/vm_exec.rs`.
- **Pleno Source Tests**: `.pln` programs in `tests/` and `examples/`, run by `scripts/run_pln_tests.sh`. Comments at the top of a file say what it must print or which error it must fail with; see [tests/README.md](tests/README.md).
- **Docs Examples**: every ` ```pleno ` block in `docs/` runs in CI, through `scripts/check-doc-examples.py`.

To run all tests, as CI does:

```bash
cargo test --workspace
cargo build --bin pleno
bash scripts/run_pln_tests.sh
python3 scripts/check-doc-examples.py target/debug/pleno docs
```

## 📝 Coding Standards

- **Formatting**: Always run `make format` before committing.
- **Linting**: We enforce strict linting. Run `make lint` to ensure no warnings or errors exist.
- **Idiomatic Rust**: Prefer idiomatic Rust. Use `clippy` suggestions.
- **Safety**: Avoid `unsafe` unless absolutely necessary and documented.
- **Public API**: Minimize the public surface area of crates. Use `pub(crate)` for internal logic.

## 🚀 Pull Request Process

1. **Open an issue** first for anything larger than a small fix, and for every change to the language or the standard library (see [GOVERNANCE.md](GOVERNANCE.md)).
2. **Fork** the repository and create your branch from `main`.
3. **Implement** your changes, with a line under "Unreleased" in `CHANGELOG.md` for anything users notice.
4. **Verify** with `make check` (this runs format, lint, and the Rust tests) and the `.pln` tests above.
5. **Commit** using [Conventional Commits](https://www.conventionalcommits.org/) (commitlint checks them):
   - `feat:` for new features
   - `fix:` for bug fixes
   - `docs:` for documentation changes
   - `refactor:` for code restructuring
6. **Push** and open a Pull Request against `main` that says `Closes #<issue>`.
7. **Respond** to review feedback promptly.

## 🔢 Versioning

Pleno follows [SemVer](https://semver.org/); see [Versioning](README.dev.md#versioning) in the developer guide. From 1.0 on, changes must keep the [compatibility promise](docs/specs/compatibility.md): the language reference is frozen for 1.x, so changes to it are additions.

## ⚖️ License

By contributing to Pleno, you agree that your contributions will be licensed under the project's **MIT License**.

---

**Happy contributing! 🚀**
