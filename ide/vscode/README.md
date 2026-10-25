# Pleno for VS Code

Language support for the [Pleno programming language](https://github.com/otabekoff/pleno): highlighting, a language server, a debugger, a test explorer, and a one-click toolchain install.

## Getting started

1. Install the extension.
2. Run **Pleno: Install or Update Toolchain** from the Command Palette. It downloads the latest Pleno release for your system from GitHub, verifies its SHA-256 checksum, and installs `pleno`, `plm`, `pleno-lsp`, `pleno-dap` and `pleno-repl`. VS Code's integrated terminals get them on PATH.
3. Open a `.pln` file and click **Run** in the editor title bar, or press `F5` to debug.

Already have Pleno installed? The extension finds it on PATH. See [Finding the toolchain](#finding-the-toolchain).

## Features

### Editing

- Syntax highlighting for `.pln` files and for ` ```pleno ` blocks in Markdown.
- Diagnostics as you type, hover, completion, signature help, inlay hints and semantic highlighting. Each problem shows where it comes from (`pleno` or `pleno-lint`) and its rule (`no-unused-vars`, `operator-spacing`, ...); unused names are faded out.
- Quick fixes (`Ctrl+.`): remove an unused import or variable, prefix an unused name with `_`, add spaces around an operator.
- Go to definition, find references, rename (keywords and built-ins are refused up front), call hierarchy and code lens.
- Highlights every use of the name under the cursor, and **Expand Selection** (`Shift+Alt+Right`) grows by name, expression and statement.
- **Go to Symbol in Workspace** (`Ctrl+T`) finds functions, classes, interfaces and top-level variables in open files and every `.pln` file in the workspace.
- Format Document (`Shift+Alt+F`) and Format Selection, which formats the top-level declarations the selection touches.
- Snippets: `fn`, `fnr`, `async`, `class`, `classex`, `constructor`, `interface`, `enum`, `let`, `lett`, `letm`, `const`, `if`, `ife`, `for`, `forr`, `while`, `match`, `import`, `importfrom`, `try`, `tryf`, `prop`, `get`, `set`, `op`, `macro`, `print`.

### Running and debugging

- **Run Current File** and **Check Current File** in the editor title bar and context menus. Files are saved first.
- Debugging with `pleno-dap`: breakpoints, stepping, call stack and variables (`F5`).
- **Open Interactive REPL**.
- Test Explorer integration for Pleno tests.

### Projects

- A **Pleno** sidebar with the project from `pleno.toml`: modules, dependencies and scripts.
- Tasks for `plm build`, `plm test`, `plm install` and `plm update`, and for running scripts from `pleno.toml`.
- **Create New Project** (`plm init`) and **Open Project Manifest**.

## Commands

| Command | What it does |
| --- | --- |
| Pleno: Install or Update Toolchain | Downloads and installs the latest release |
| Pleno: Uninstall Toolchain | Removes the toolchain installed by the extension |
| Pleno: Run Current File | `pleno run` on the active file |
| Pleno: Check Current File | `pleno check` on the active file |
| Pleno: Start Debugging | Debugs the active file |
| Pleno: Open Interactive REPL | Starts `pleno-repl` in a terminal |
| Pleno: Restart Language Server | Restarts `pleno-lsp` |
| Pleno: Show Pleno Status | Prints where each tool was found |
| Pleno: Create New Project | `plm init` in the workspace folder |

Click **Pleno** in the status bar for a menu of these. The status bar item also shows whether the language server is starting, running or stopped.

## Finding the toolchain

For each tool the extension uses, in order:

1. The setting: `pleno.cli.path`, `pleno.lsp.path` or `pleno.dap.path`.
2. The toolchain installed by **Install or Update Toolchain**.
3. Your PATH.
4. A build in the Pleno repository (`target/release` or `target/debug`), when you work on Pleno itself.

Changing a path setting restarts the language server. Without `pleno-lsp`, diagnostics (on open and save) and formatting fall back to the `pleno` command.

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `pleno.cli.path` | empty | Path to `pleno` |
| `pleno.lsp.path` | `pleno-lsp` | Path to the language server |
| `pleno.dap.path` | empty | Path to the debug adapter |
| `pleno.inlayHints.enabled` | `true` | Show inlay hints |
| `pleno.semanticHighlighting.enabled` | `true` | Use semantic highlighting |
| `pleno.lint.enabled` | `true` | Report lint warnings as you type |
| `pleno.lint.maxLineLength` | `100` | Line length limit for `max-line-length` (`0` turns it off) |
| `pleno.lint.rules` | `{}` | Rules to turn off, e.g. `{ "operator-spacing": false }` |
| `pleno.trace.server` | `off` | Log language server traffic |

Lint settings apply as soon as you change them. The rules are `no-unused-vars`, `no-undef`, `no-shadow`, `operator-spacing`, `max-line-length`, `no-trailing-spaces` and `semi-spacing`.

## File icons

`.pln` files show the Pleno icon in any icon theme that has no icon of its own for them. For Pleno icons everywhere, select **Pleno** under **Preferences: File Icon Theme**.

## Links

- [Documentation](https://pleno.mohirlab.uz/)
- [Language specification](https://github.com/otabekoff/pleno/blob/main/notes/specs.md)
- [Report an issue](https://github.com/otabekoff/pleno/issues)

## License

MIT
