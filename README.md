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

## CLI Usage

### `pleno` - Language Runner & Tools

- `pleno run <file>`: Execute a Pleno script.
- `pleno repl`: Interactive shell.
- `pleno format <path>`: Auto-format your code.
- `pleno lint <path>`: Check for common issues.
- `pleno test <path>`: Run the built-in test suite.
- `pleno doc <path>`: Generate API documentation.

### `plm` - Pleno Module Manager

`plm` is the built-in package manager for handling dependencies and project lifecycle.

- `plm init [name]`: Initialize a new project.
- `plm install [url]`: Add a dependency.
- `plm update`: Update dependencies.
- `plm build`: Compile the project.
- `plm run <script>`: Run custom scripts from `pleno.toml`.



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

## Documentation

- [Language Specification](specs.md)
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)

## License

Pleno is released under the MIT License. See [LICENSE](LICENSE) for details.
