# Development Guide - Pleno

This guide provides instructions for building, testing, and contributing to the Pleno programming language.

## Prerequisites
- Rust (latest stable)
- Node.js & npm (for documentation)

## Building the Project
We use a `Makefile` to simplify common tasks.

```bash
# Build all crates (VM, Compiler, Linter, etc.)
make build

# Clean build artifacts
make clean
```

## Testing
Always run tests before submitting changes.

```bash
# Run all tests in the workspace
make test

# Run linter
make lint
```

## Manual Verification
You can test the language features manually using the REPL or by running `.pln` files.

### Using the REPL
```bash
cargo run -p pleno-repl
```

### Running a File
```bash
cargo run -p pleno -- run examples/hello.pln
```

## Documentation
The documentation is built with Vitepress and located in the `docs/` directory.

```bash
# Start local development server
make docs-dev

# Build static site
make docs-build
```

## IDE Support (VSCode)
We provide a VSCode extension for syntax highlighting in `ide/vscode`.

1. Open the `ide/vscode` directory in VSCode.
2. Press `F5` to open a new "Extension Development Host" window.
3. In the new window, open any `.pln` file to see the syntax highlighting in action.

Alternatively, you can link the directory to your extensions folder:
```bash
ln -s $(pwd)/ide/vscode ~/.vscode/extensions/pleno-vscode
```

## Troubleshooting
### Flycheck Errors
If you see errors about duplicated `--all-targets` flags, check `.vscode/settings.json` and ensure `rust-analyzer.checkOnSave.extraArgs` does not contain flags already enabled by `rust-analyzer.checkOnSave.allTargets`.
