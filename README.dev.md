# Pleno Developer Guide

How the repository is organized, how changes get in, and how releases are made.

## 📖 Navigation

- **[Architecture Deep Dive](ARCHITECTURE.md)**: Detailed technical overview of VM and Compiler internals.
- **[Language Reference](docs/specs/)**: The authoritative reference for Pleno syntax and semantics; [compatibility promise](docs/specs/compatibility.md); designs for later in [notes/v2-draft.md](notes/v2-draft.md).
- **[Contributing Guidelines](CONTRIBUTING.md)**: How to submit your changes.
- **[Code signing policy](docs/code-signing-policy.md)**: how releases are signed and verified.
- **[What is left before 1.0](ROADMAP.md)**, with the details in [`tasks/`](tasks/README.md).

---

## 🚀 Getting Started

### Prerequisites

- **Rust**, the latest stable version (the workspace uses edition 2024, so 1.85 or newer).
- **Make**, for the shortcuts below (optional: each target is one or two commands).
- **Node.js 22** for the VS Code extension in `ide/vscode`, and **[Bun](https://bun.sh)** for the
  docs site and the commit hooks.
- **Python 3** for `scripts/check-doc-examples.py` and `scripts/sync_project.py`.

### Local Setup

```bash
git clone https://github.com/otabekoff/pleno.git
cd pleno
bun install          # commit hooks (Husky, commitlint) and the docs site
cargo build --workspace
```

`make setup` adds clippy and rustfmt and installs the npm packages; `make build` builds the
workspace.

---

## 🏗️ Project Breakdown

The crates are listed in the [README](README.md#-workspace-layout). Other folders:

| Folder | What it holds |
| --- | --- |
| `docs/` | The documentation site (VitePress), including the language reference |
| `ide/vscode/` | The VS Code extension |
| `tests/`, `examples/` | Pleno programs run by `scripts/run_pln_tests.sh` ([tests/README.md](tests/README.md)) |
| `fuzz/` | `cargo fuzz` targets, run by `.github/workflows/fuzz.yml` |
| `bench/` | Benchmarks and the comparison with other languages |
| `packaging/` | Package manager manifests (Homebrew, Scoop, WinGet, Chocolatey, AUR, ...) |
| `scripts/` | Install scripts and the helpers CI and releases use |
| `tasks/`, `notes/` | Open work, and reference notes that are not task lists |

---

## 🌿 Branch Management

- **`main`** must always build and pass CI; it is what releases are tagged from. Nobody commits
  to it directly.
- Work happens on a branch named after it: `feat/...`, `fix/...`, `docs/...`, `ci/...`,
  `chore/...`.

### Workflow

1. Open an issue that describes the change (for language changes, before writing code).
2. Branch from `main`: `git switch -c fix/short-name origin/main`.
3. Implement and test (see [CONTRIBUTING.md](CONTRIBUTING.md#-testing-guidelines)).
4. Push and open a pull request to `main` that says `Closes #<issue>`. CI must pass.

---

## 📝 Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/), checked by commitlint in
the `commit-msg` hook:

`type(scope): description`

**Types:**

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting, missing semicolons, etc.
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `perf`: Performance improvement
- `test`: Adding missing tests
- `build`, `ci`: Build system and CI
- `chore`: Maintenance

A `!` after the type (`feat!:`) marks a breaking change.

The `pre-commit` hook runs `cargo fmt --check`, `cargo clippy`, `cargo test` and `cargo build` on
the workspace.

---

## 📦 Release Procedure & Tags

### Versioning

Pleno follows [SemVer](https://semver.org/). Before 1.0, a minor release (0.x.0) may change
behavior; from 1.0 on, the [compatibility promise](docs/specs/compatibility.md) applies. The
crates and the VS Code extension share one version.

### Making a release

1. Set the new version in `pleno.json` and run `make sync` (or `python3 scripts/sync_project.py`):
   it updates the workspace version in `Cargo.toml`, the extension's `package.json`,
   `SECURITY.md` and `LICENSE`. Then run `cargo build` and `npm install` in `ide/vscode`, so that
   `Cargo.lock`, `fuzz/Cargo.lock` (`cargo update --manifest-path fuzz/Cargo.toml --workspace`)
   and `ide/vscode/package-lock.json` follow.
2. Date the release in `CHANGELOG.md` and `ide/vscode/CHANGELOG.md`.
3. Merge that through a pull request, then tag the merge commit on `main`:
   `git tag vX.Y.Z && git push origin vX.Y.Z`.
4. `.github/workflows/release.yml` builds every target, signs the assets with cosign, publishes
   the GitHub release and the Docker image, and updates Homebrew, Scoop, WinGet and the
   extension stores. A tag with a `-` (`v1.0.0-rc.1`) is a prerelease and skips the package
   managers and stores. Channels that are updated by hand are listed in
   [tasks/distribution.md](tasks/distribution.md) and [packaging/README.md](packaging/README.md).

### Redoing a failed release

If the release workflow fails before publishing, fix the problem on `main` and move the tag:

```bash
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
git tag vX.Y.Z    # on the fixed commit
git push origin vX.Y.Z
```

Once a release is published (and package managers have picked it up), do not move its tag;
make a new patch release instead.

### Trust & Security

- **Every asset** has a `.sha256` file and a Sigstore (cosign) bundle, and `SHA256SUMS.txt` lists
  them all: `sha256sum -c SHA256SUMS.txt`.
- **Windows**: code signing through [SignPath](https://signpath.org) is applied for but not active
  yet, so SmartScreen warns on first run.
- **macOS**: Homebrew installs the archives without the Gatekeeper prompt.

See the [Code signing policy](docs/code-signing-policy.md) for details.

---

## 💡 Troubleshooting & Fixes

### CI Failures

- **Hygiene**: `make check` runs `cargo fmt`, `cargo clippy` and `cargo test`, as CI does.
- **Lockfiles**: CI checks that `Cargo.lock` and `fuzz/Cargo.lock` are up to date; after changing
  a dependency, run `cargo update --manifest-path fuzz/Cargo.toml --workspace`.
- **Pre-commit**: the hooks live in `.husky/`; `bun install` sets them up. If they fail, fix the
  problem rather than skipping them.

### VM Issues

- **Bytecode**: `pleno compile script.pln` writes `script.plc`; `pleno run script.plc` runs it.
- **Interpreter loops**: the fuzz target `fuzz_run` runs each program with the fast and the general
  loop and fails when they disagree.

### Fixing Corrupt State

If `cargo` gets into a weird state:

```bash
cargo clean
```
