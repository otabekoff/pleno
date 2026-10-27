# Contributing

This page explains how to build Pleno from source, run the same checks as CI, and send a change. Changes of every size are welcome: a typo in these docs, a test, a bug fix or a new feature. Everyone with a merged change is listed on the [team and contributors](/team) page.

For a change to the language or the standard library, open an [issue](https://github.com/otabekoff/pleno/issues) first that describes the problem and the behavior you propose, so it can be discussed before you write the code. The 1.x language reference is frozen: additions are possible, changes to documented behavior are not (see the [compatibility promise](/specs/compatibility)).

## What you need

- [Rust](https://rustup.rs), stable, with `rustfmt` and `clippy` (`rustup component add rustfmt clippy`).
- Git.
- [Bun](https://bun.sh), for the commit hooks and the documentation site.
- Python 3, for the script that runs the examples in these docs.
- Node.js, for the VS Code extension in `ide/vscode` and the playground's smoke test.

## Build and run

```sh
git clone https://github.com/otabekoff/pleno.git
cd pleno
bun install                      # the commit hooks (husky, commitlint)
cargo build --release -p pleno-cli
target/release/pleno run examples/hello.pln
```

`pleno-cli` builds the `pleno` and `plm` programs. `cargo build --workspace` builds everything, including `pleno-lsp`, `pleno-dap` and `pleno-repl`. While working, `cargo run -p pleno-cli --bin pleno -- run file.pln` builds and runs in one step.

## The repository

| Path | What is in it |
| --- | --- |
| `crates/pleno-lexer`, `pleno-parser`, `pleno-ast` | Source text to tokens to a syntax tree. |
| `crates/pleno-compiler`, `pleno-optimizer`, `pleno-bytecode` | The syntax tree to bytecode, and the bytecode format. |
| `crates/pleno-vm`, `pleno-value` | The register-based virtual machine, its values, the garbage collector and the event loop. |
| `crates/pleno-stdlib` | The `std:*` modules. |
| `crates/pleno-cli` | The `pleno` and `plm` commands. |
| `crates/pleno-package` | Packages: `pleno.toml`, `pleno.lock`, fetching from Git. |
| `crates/pleno-formatter`, `pleno-linter`, `pleno-doc` | `pleno format`, `pleno lint`, `pleno doc`. |
| `crates/pleno-lsp`, `pleno-dap`, `pleno-repl` | The language server, the debugger and the REPL. |
| `crates/pleno-wasm` | The WebAssembly build used by the playground and the tutorial. |
| `crates/pleno-common`, `pleno-diagnostics` | Shared code and error reporting. |
| `tests/`, `examples/` | `.pln` programs that CI runs and checks. |
| `fuzz/` | Fuzz targets for the lexer, the parser, the compiler and the VM. |
| `bench/` | Benchmarks, compared with CPython. |
| `docs/` | This site: the guide, the tutorial and the language and standard library reference. |
| `ide/vscode/` | The VS Code extension. |
| `tasks/` | What is left before 1.0 ([`tasks/v1.md`](https://github.com/otabekoff/pleno/blob/main/tasks/v1.md)). |

[ARCHITECTURE.md](https://github.com/otabekoff/pleno/blob/main/ARCHITECTURE.md) describes how the parts fit together.

## The checks

CI runs these on every pull request. Run the ones that concern your change before you push:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
bash scripts/run_pln_tests.sh target/debug/pleno     # the .pln programs in tests/ and examples/
bash scripts/test_http_server.sh                     # the HTTP server, over real connections
python3 scripts/check-doc-examples.py target/debug/pleno docs   # every example in these docs
```

`make check` runs the formatter, clippy and the Rust tests in one go.

A `.pln` file in `tests/` or `examples/` is run by `run_pln_tests.sh`, which reads directives from its first lines: `// expect-error: <text>`, `// args: <flags>`, `// stdin: <text>`, `// skip: <reason>` and `// requires: network`. Add one for each bug you fix.

## Changing the docs

The site uses [VitePress](https://vitepress.dev). The playground and the tutorial run Pleno compiled to WebAssembly, which you build once, and again after changing the Rust code:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.108   # the version in Cargo.lock
bun run docs:wasm
bun run docs:dev       # a local server that reloads as you edit
bun run docs:build     # what the deployment runs; a broken internal link fails it
```

Every ```` ```pleno ```` block in `docs/` is run by `check-doc-examples.py`, and the build adds a **Run** link to it. A block that cannot run by itself (a fragment, a server, a file that imports another) is marked ```` ```pleno norun ````. `node scripts/playground-smoke.mjs` runs a few programs through the WebAssembly build.

Write plain, concrete English, and describe what exists: features for later versions belong in [`notes/v2-draft.md`](https://github.com/otabekoff/pleno/blob/main/notes/v2-draft.md), not in the reference.

## Send a change

1. Fork the repository and create a branch from `main`, named after the change (`fix/slice-bounds`, `docs/http-cookies`).
2. Make the change, with a test for a bug fix or a feature, and update the docs it affects.
3. Add a line to [`CHANGELOG.md`](https://github.com/otabekoff/pleno/blob/main/CHANGELOG.md) under **Unreleased** when users can notice the change.
4. Commit with a [Conventional Commits](https://www.conventionalcommits.org/) message: `feat: ...`, `fix: ...`, `docs: ...`, `test: ...`, `refactor: ...`, `chore: ...`. The `commit-msg` hook checks it with commitlint, and the `pre-commit` hook formats Rust files and runs `cargo fmt --check`, clippy, the tests and the build.
5. Push the branch and open a pull request against `main` that says what changed and why, and links the issue (`Closes #123`).

The maintainer reviews every pull request. Pleno is maintained in spare time, so a review can take a few days.

## Debugging tips

- `RUST_LOG=debug pleno run file.pln` prints the internal log; `RUST_LOG=pleno_vm=trace` narrows it to the VM.
- `RUST_BACKTRACE=1` adds a Rust backtrace to a crash.
- `pleno check file.pln` reports compile errors and other problems without running the file.
- `pleno compile file.pln` writes the bytecode to a `.plc` file.

## License

Pleno is released under the [MIT License](https://github.com/otabekoff/pleno/blob/main/LICENSE). By contributing, you agree that your contributions are released under it too. Everyone taking part follows the [Code of Conduct](https://github.com/otabekoff/pleno/blob/main/CODE_OF_CONDUCT.md).
