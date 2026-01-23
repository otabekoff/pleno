# Pleno Language

Rust workspace for the Pleno programming language implementation.

## Workspace Layout

- `crates/pleno-ast` - AST nodes and spans
- `crates/pleno-lexer` - lexer/tokenizer
- `crates/pleno-parser` - parser producing AST
- `crates/pleno-runtime` - runtime stubs
- `crates/pleno-cli` - CLI entry point

## Quick Start

```bash
cargo run -p pleno-cli -- path/to/file.pln
```

## Example

```bash
cargo run -p pleno-cli -- examples/hello.pln
```

## Roadmap

See `ROADMAP.md` for staged milestones toward v1.