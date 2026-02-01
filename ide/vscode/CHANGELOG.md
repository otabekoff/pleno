# Change Log

All notable changes to the "pleno" extension will be documented in this file.

Check [Keep a Changelog](http://keepachangelog.com/) for recommendations on how to structure this file.


## [0.1.0] - 2026-02-01

### Added

- Comprehensive syntax highlighting for all Pleno keywords and operators.
- New snippets for `property`, `operator`, `macro`, `getter/setter`, etc.
- Explicit `onLanguage:pleno` activation for reliable LSP startup.
- Extension settings for custom `pleno-lsp` path and tracing.
- Commands: `Pleno: Run Current File`, `Pleno: Check Current File`, `Pleno: Restart Language Server`.
- `onEnterRules` for doc comments (`///`) and smart indentation rules.

### Changed

- Improved activation alerts with LSP location info.
- Cleaned up icon themes (removed redundant Material theme).
- Expanded supported file extensions to include `.pln`.

## [0.0.1] - 2026-01-15

- Initial release
