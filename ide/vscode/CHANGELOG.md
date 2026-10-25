# Changelog

All notable changes to the Pleno extension for VS Code. The format follows
[Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- Diagnostics show their source (`pleno`, `pleno-lint`) and rule code, cover the whole offending name or operator, and unused names are faded out.
- Quick fixes: remove an unused import or variable, prefix an unused name with `_`, and add spaces around an operator.
- Highlighting of every use of the name under the cursor, **Go to Symbol in Workspace**, **Expand Selection**, and **Format Selection**.
- Rename refuses keywords, built-ins and non-names before asking for the new name.
- Settings `pleno.lint.enabled`, `pleno.lint.maxLineLength` and `pleno.lint.rules`, applied without a restart.
- The status bar item shows when the language server is starting or has stopped.
- **Pleno: Debug File** in the editor title bar's run menu and the context menus, and
  **Pleno: Run Script...** to pick a script from `pleno.toml`.
- Snippets `lambda`, `catcht` (catch by type), `describe` and `test`.
- `plm run <script>` tasks for every script in `pleno.toml`.
- `pleno.toml` completion and validation with the Even Better TOML extension.
- The toolchain is also found in `~/.pleno/bin`, where the install scripts put it.
- Without `pleno-dap`, debugging uses `pleno dap` instead of a simulated session.

### Changed

- The Testing view finds `describe(...)` and `test(...)` from `std:test`, nested as in the source,
  and shows each test's result and failure message from `pleno test`. It used to guess tests from
  function names and pass flags (`--filter`, `--coverage`) that `pleno test` does not have.
- The project view moved into the Explorer as **Pleno Project**, shown when the workspace has a
  `pleno.toml`. It reads the manifest with a TOML parser and lists the entry file, scripts and
  dependencies, and it updates when `pleno.toml` changes.
- **Pleno: Show Status** (also the status bar item) is a menu with each tool's version, location
  and the language server state, and the common actions. It replaces **Show Control Panel**,
  **Show Output Channel** and the status report in the output.
- **Pleno: New Project...** asks for a folder when the workspace has none without a project,
  and opens the new project.
- Commands are named for what they act on: **Run File**, **Check File**, **Open REPL**,
  **Open pleno.toml**. Commands that need a Pleno file or project are hidden elsewhere.
- Launch configurations take `program`, `cwd` and `env`, which is what `pleno-dap` supports; a
  relative `program` is resolved against the workspace folder.
- The walkthrough has one step per task (toolchain, project, run, tests, docs), and steps are
  checked off when done.
- Highlighting follows the lexer: template and raw strings, `${...}` in strings, character
  literals, hex, binary and octal numbers, `_` separators, exponents, `@annotations`, and function,
  class and constant names. Words that are not Pleno keywords are no longer highlighted.

### Fixed

- Find references, rename and highlights from a function name or parameter found only the declaration.
- Run and Check from the Explorer context menu ran the active editor's file instead of the clicked one.
- ` ```pleno norun ` and other fences with an info string were not highlighted in Markdown.

### Removed

- The dashboard webview, which showed a hard-coded version and links to pages that do not exist.
- The custom **Outline** view; VS Code's **Outline** shows the language server's symbols.
- **Reload Workspace**, **Refresh Project View**, **Show Control Panel**, **Show Output Channel**
  and **Start Debugging** (now **Debug File**).
- The **Pleno** file icon theme, which replaced every other file's icon and claimed `.pl` files.
  `.pln` files keep their icon in any theme.
- The `attach` debug configuration and `stopOnEntry` and `args`, which `pleno-dap` does not support.
- The `pleno.inlayHints.enabled` and `pleno.semanticHighlighting.enabled` settings, which had no
  effect; use `editor.inlayHints.enabled` and `editor.semanticHighlighting.enabled` for `[pleno]`.
- JSON validation for `pleno.toml`, which never applied to a TOML file.

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
