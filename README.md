# Pleno Language

Rust workspace for the Pleno programming language implementation.

## Workspace Layout

- `crates/pleno-ast` - AST nodes and spans
- `crates/pleno-lexer` - lexer/tokenizer
- `crates/pleno-parser` - parser producing AST
- `crates/pleno-runtime` - runtime stubs
- `crates/pleno-cli` - CLI entry point

## Quick Start

### Installation

For Linux and macOS:
```bash
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh
```

### Usage

```bash
# Run a file
pleno run path/to/file.pln

# For contributors (running from source):
cargo run --release -- path/to/file.pln
```

## Example

```bash
pleno run examples/hello.pln
```

## Roadmap

See `ROADMAP.md` for staged milestones toward v1.

## Release Process

To create a new release (triggers Windows, Linux, and macOS builds):

1.  **Tag the release**:
    ```bash
    git tag v0.1.0
    git push origin v0.1.0
    ```

2.  **Redo a release** (if something failed and you need to re-trigger for the same version):
    ```bash
    # Delete local tag
    git tag -d v0.1.0
    # Delete remote tag
    git push origin :refs/tags/v0.1.0

    # ... commit fixes ...

    # Re-tag and push
    git tag v0.1.0
    git push origin v0.1.0
    ```
