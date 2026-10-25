# Change Log

All notable changes to the "pleno" extension will be documented in this file.

Check [Keep a Changelog](http://keepachangelog.com/) for recommendations on how to structure this file.


## [Unreleased]

### Added

- Diagnostics show their source (`pleno`, `pleno-lint`) and rule code, cover the whole offending name or operator, and unused names are faded out.
- Quick fixes: remove an unused import or variable, prefix an unused name with `_`, and add spaces around an operator.
- Highlighting of every use of the name under the cursor, **Go to Symbol in Workspace**, **Expand Selection**, and **Format Selection**.
- Rename refuses keywords, built-ins and non-names before asking for the new name.
- Settings `pleno.lint.enabled`, `pleno.lint.maxLineLength` and `pleno.lint.rules`, applied without a restart.
- The status bar item shows when the language server is starting or has stopped.

### Fixed

- Find references, rename and highlights from a function name or parameter found only the declaration.

## [0.2.0] - 2026-10-24

### Added

- **Pleno: Install or Update Toolchain** downloads the latest release for your system from GitHub, verifies its SHA-256 checksum, and installs it; integrated terminals get it on PATH. **Pleno: Uninstall Toolchain** removes it.
- When the toolchain is missing, the extension offers to install it.
- **Pleno: Show Pleno Status** shows where each tool was found.

### Fixed

- The status bar reported the toolchain as found even when it was missing.
- The `pleno.cli.path` setting was ignored, and local builds were not found on Windows.
- Formatting used the file on disk and discarded unsaved changes.
- With the language server running, diagnostics were reported twice and two formatters were registered. The CLI is now only a fallback.
- Run, check, REPL, script and new-project commands failed for paths with spaces in PowerShell; they now start the tool directly. Run saves the file first and no longer types into whatever terminal was active.
- Snippets produced code that does not parse (`-> Type`, `mut x`, `catch e`, `while x`, `import x;`, `property` without a body, `println`).
- `async`, `await` and `out` are highlighted.
- The documentation link and the getting-started walkthrough (wrong syntax and links).

### Removed

- Patching other extensions' icon themes. Themes patched by earlier versions are restored. `.pln` files get their icon from the language contribution.
- The bracket-counting syntax checker, which duplicated the language server and misread braces inside interpolations, and its quick fixes.

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
