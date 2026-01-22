# Contributing to Pleno

Thanks for helping build Pleno.

## Getting Started

- Fork the repo and create a feature branch.
- Keep changes focused and include tests when possible.
- Run `cargo fmt` and `cargo clippy` before opening a PR.

## Development Flow

- Workspace crates live in `crates/`.
- `tests/` is for integration and cross-crate tests.
- Per-crate tests live in `crates/<crate>/tests`.

## Reporting Issues

Please include:
- Expected vs actual behavior
- Reproduction steps
- Environment details (OS, Rust version)

## Code Style

- Prefer clear naming and small modules.
- Avoid public APIs unless needed; use `pub(crate)`.

## License

By contributing, you agree that your contributions will be licensed under the
MIT License.
