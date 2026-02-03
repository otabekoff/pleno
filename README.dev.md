# Pleno Developer Guide

Welcome to the Pleno internal development guide. This document defines our engineering standards, branching strategy, and release procedures.

## 📖 Navigation

- **[Architecture Deep Dive](ARCHITECTURE.md)**: Detailed technical overview of VM and Compiler internals.
- **[Language Specification](specs.md)**: The authoritative reference for Pleno syntax and semantics.
- **[Contributing Guidelines](CONTRIBUTING.md)**: How to submit your changes.
- **[Code Signing Guide](docs/security/signing.md)**: professional signing and security procedures.

---

## 🚀 Getting Started

### Prerequisites

- **Rust 1.82+**: We use modern features like `let_chains`.
- **Make**: Standard build tool.
- **Docker**: For containerized development and testing.

### Local Setup

```bash
# Clone the repo
git clone https://github.com/otabekoff/pleno.git
cd pleno

# Initial setup
make setup

# Build everything
make build
```

---

## 🏗️ Project Breakdown

Pleno is a workspace of modular crates:

| Crate            | Purpose                                     |
| ---------------- | ------------------------------------------- |
| `pleno-cli`      | The entry point (`pleno` command).          |
| `pleno-ast`      | Shared AST nodes and source span utilities. |
| `pleno-lexer`    | Lexical scanner.                            |
| `pleno-parser`   | Syntax analyzer (Recursive Descent).        |
| `pleno-compiler` | Bytecode generation.                        |
| `pleno-bytecode` | Instruction definitions.                    |
| `pleno-vm`       | Core execution engine.                      |
| `pleno-stdlib`   | Standard library.                           |
| `pleno-registry` | Package registry backend.                   |

---

## 🌿 Branch Management

We follow a strict branching model to maintain stability:

- **`main`**: **Production Stable**. This branch must always pass all tests and be ready for release. Direct commits are strictly prohibited.
- **`dev`**: **Integration Branch**. Work-in-progress is merged here.
- **`feature/*`**: New features.
- **`fix/*`**: Bug fixes.
- **`docs/*`**: Documentation updates.

### Workflow

1. Create a branch from `dev`: `git checkout dev && git checkout -b feature/cool-thing`
2. Implement and test: `make check`
3. Push and open PR to `dev`.
4. Once `dev` is stable, it is merged into `main` for release.

---

## 📝 Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

`type(scope): description`

**Types:**

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting, missing semi colons, etc
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `perf`: Performance improvement
- `test`: Adding missing tests
- `chore`: Maintenance

---

## 📦 Release Procedure & Tags

### Versioning

We use **SemVer 0.1.0** for the current stage.

### Tag Management (Fixing Releases)

If a release fails and you need to retag `v0.1.0`:

1.  **Remove local tag**: `git tag -d v0.1.0`
2.  **Remove remote tag**: `git push origin :refs/tags/v0.1.0`
3.  **Fix the issue** on `dev`, merge to `main`.
4.  **Re-tag**: `git tag v0.1.0`
5.  **Push**: `git push origin v0.1.0`

### Trust & Security (Open Source Model)
Pleno is a non-profit project. Instead of expensive commercial certificates, we use a **Community Trust Model**:

- **Windows**: [SignPath.io](https://signpath.io/) (Free for OSS) or reputation building.
- **macOS**: Distribution via **Homebrew** to bypass Gatekeeper.
- **Linux**: **GPG signatures** and SHA256 checksums (Free).

See the **[Security & Trust Guide](docs/security/signing.md)** for setup details.

All releases include a `SHA256SUMS.txt` file. Verify with:
`sha256sum -c SHA256SUMS.txt`

---

## 💡 Troubleshooting & Fixes

### CI Failures

- **Hygiene**: Run `make check`. This runs `cargo fmt`, `cargo clippy`, and `cargo test`.
- **Pre-commit**: We use Husky. If it fails, fix the lint errors before pushing.

### VM Issues

- **Trace**: `RUST_LOG=pleno_vm=trace pleno run script.pln`
- **Bytecode**: `pleno compile --dump-bytecode script.pln`

### Fixing Corrupt State

If `cargo` or `make` gets into a weird state:

```bash
make clean
rm -rf target/
cargo update
```

---

**Pleno Team** | _Crafted with absolute precision._
