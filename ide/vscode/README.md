# Pleno for VS Code

Support for the [Pleno programming language](https://pleno.mohirlab.uz/): highlighting, the
`pleno-lsp` language server, the `pleno-dap` debugger, tests in the Testing view, and an installer
for the toolchain.

## Getting started

1. Install the extension.
2. Run **Pleno: Install or Update Toolchain** from the Command Palette (`Ctrl+Shift+P`). It
   downloads the latest Pleno release for your system from GitHub, checks its SHA-256 checksum, and
   installs `pleno`, `plm`, `pleno-lsp`, `pleno-dap` and `pleno-repl`. VS Code's integrated
   terminals get them on `PATH`. Skip this if Pleno is already installed.
3. Open a `.pln` file and click **Run File** in the editor title bar, or press `F5` to debug it.

**Help: Welcome** has a **Get Started with Pleno** walkthrough with the same steps.

## Features

### Editing

- Highlighting for `.pln` files and for ` ```pleno ` blocks in Markdown.
- Diagnostics as you type, hover, completion, signature help, inlay hints and semantic
  highlighting. Each problem shows its source (`pleno` or `pleno-lint`) and rule; unused names are
  faded out.
- Quick fixes (`Ctrl+.`): remove an unused import or variable, prefix an unused name with `_`, add
  spaces around an operator.
- Go to definition, find references, rename, call hierarchy, document highlights, **Expand
  Selection**, the **Outline** view and **Go to Symbol in Workspace** (`Ctrl+T`).
- **Format Document** (`Shift+Alt+F`) and **Format Selection**.
- Snippets: `fn`, `fnr`, `async`, `lambda`, `class`, `classex`, `constructor`, `prop`, `op`,
  `interface`, `enum`, `let`, `lett`, `letm`, `const`, `if`, `ife`, `for`, `forr`, `while`, `match`,
  `import`, `try`, `tryf`, `with`, `catcht`, `print`, `describe` and `test`.

### Running, debugging and testing

- **Run File** and **Debug File** in the editor title bar's run menu, and with **Check File** in
  the editor and Explorer context menus. Files are saved first.
- Debugging with breakpoints (conditional ones too), stepping, the call stack, variables and
  watch expressions. A `launch.json` configuration takes `program`, `cwd` and `env`.
- Tests written with `std:test` (`describe` and `test`) appear in the **Testing** view, with each
  test's result and failure message. A file always runs as a whole, as `pleno test <file>` does.
- **Pleno: Open REPL** starts `pleno-repl` in a terminal.

### Projects

- The **Pleno Project** view in the Explorer shows the package from `pleno.toml`: its version,
  entry file, scripts and dependencies. Run a script with the play button next to it, or with
  **Pleno: Run Script...**.
- **Tasks: Run Task** has `plm build`, `plm build --release`, `plm test`, `plm install`,
  `plm update` and `plm run <script>` for every script in `pleno.toml`.
- **Pleno: New Project...** runs `plm init` in a folder you pick.
- With the [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml)
  extension, `pleno.toml` gets completion and validation from the manifest schema.

## Commands

| Command | What it does |
| --- | --- |
| Pleno: Run File | `pleno run` on the file, in the terminal |
| Pleno: Debug File | Debugs the file |
| Pleno: Check File | `pleno check` on the file |
| Pleno: Run Script... | `plm run` with a script from `pleno.toml` |
| Pleno: Open REPL | Starts `pleno-repl` in a terminal |
| Pleno: New Project... | `plm init` in a folder |
| Pleno: Open pleno.toml | Opens the project manifest |
| Pleno: Restart Language Server | Finds the tools again and restarts `pleno-lsp` |
| Pleno: Install or Update Toolchain | Downloads and installs the latest release |
| Pleno: Uninstall Toolchain | Removes the toolchain the extension installed |
| Pleno: Show Status | Each tool's version and location, and the server state |
| Pleno: Open Documentation | Opens [pleno.mohirlab.uz](https://pleno.mohirlab.uz/) |
| Pleno: Open Walkthrough | Opens the getting-started walkthrough |

The **Pleno** item in the status bar shows whether the language server is starting, running or
stopped; click it for **Show Status**.

## Finding the toolchain

For each tool, in order:

1. The setting: `pleno.cli.path`, `pleno.lsp.path` or `pleno.dap.path`.
2. The toolchain installed by **Install or Update Toolchain**.
3. `PATH`, then `~/.pleno/bin` (or `$PLENO_HOME/bin`), where the install scripts put it.
4. A build in the Pleno repository (`target/release` or `target/debug`), when you work on Pleno
   itself.

Changing a path setting restarts the language server. Without `pleno-lsp`, diagnostics (on open
and save) and formatting use the `pleno` command instead, and without `pleno-dap` the debugger
runs as `pleno dap`.

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `pleno.cli.path` | empty | Path to `pleno` |
| `pleno.lsp.path` | empty | Path to the language server |
| `pleno.dap.path` | empty | Path to the debug adapter |
| `pleno.lint.enabled` | `true` | Report lint warnings as you type |
| `pleno.lint.maxLineLength` | `100` | Line length limit for `max-line-length` (`0` turns it off) |
| `pleno.lint.rules` | `{}` | Rules to turn off, e.g. `{ "operator-spacing": false }` |
| `pleno.trace.server` | `off` | Log language server traffic to the Pleno output |

Lint settings apply as soon as you change them. The rules are `no-unused-vars`, `no-undef`,
`no-shadow`, `operator-spacing`, `max-line-length`, `no-trailing-spaces` and `semi-spacing`.

Inlay hints and semantic highlighting follow VS Code's own settings. To turn them off for Pleno
only:

```json
"[pleno]": {
    "editor.inlayHints.enabled": "off",
    "editor.semanticHighlighting.enabled": false
}
```

## Links

- [Documentation](https://pleno.mohirlab.uz/) and [editor setup](https://pleno.mohirlab.uz/guide/editor)
- [Source code](https://github.com/otabekoff/pleno/tree/main/ide/vscode)
- [Report an issue](https://github.com/otabekoff/pleno/issues)

## License

MIT
