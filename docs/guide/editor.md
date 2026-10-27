# Editor setup

## VS Code

The Pleno extension adds highlighting, the language server, the debugger and a test view.

1. Download `pleno-language-<version>.vsix` from the [latest release](https://github.com/otabekoff/pleno/releases) and install it:

   ```sh
   code --install-extension pleno-language-<version>.vsix
   ```

   The extension is not on the VS Code Marketplace yet.

2. Run **Pleno: Install or Update Toolchain** from the Command Palette (`Ctrl+Shift+P`). Skip this if Pleno is already on your `PATH`.

You get:

- Errors as you type, hover information, completion, signature help and inlay hints.
- Go to definition, find references, rename, and **Format Document** (`Shift+Alt+F`).
- **Run File** and **Debug File** in the editor title bar for `.pln` files; press `F5` to debug with breakpoints.
- `std:test` tests in the **Testing** view, and a **Pleno Project** view in the Explorer with the scripts and dependencies from `pleno.toml`.
- Snippets such as `fn`, `class`, `match`, `tryf` and `test`.

**Pleno: Show Status**, or the **Pleno** item in the status bar, shows the version and location of each tool the extension uses. To use specific builds, set `pleno.cli.path`, `pleno.lsp.path` and `pleno.dap.path`.

### Code Runner

The [Code Runner](https://marketplace.visualstudio.com/items?itemName=formulahendry.code-runner) extension does not know Pleno, and the Pleno extension does not change Code Runner's settings for you. Add Pleno to Code Runner's `code-runner.executorMap` in your user `settings.json` (**Preferences: Open User Settings (JSON)**):

```json
{
  "code-runner.executorMap": {
    "pleno": "cd $dir && pleno run $fileName"
  }
}
```

The key `pleno` is the language ID the Pleno extension gives `.pln` files. Without the Pleno extension, key the command by file extension instead, with `"code-runner.executorMapByFileExtension": { ".pln": "cd $dir && pleno run $fileName" }`. `cd $dir` makes relative paths in the program, and imports from `sources/`, resolve the way they do with `pleno run` in a terminal. If `pleno` is not on your `PATH`, write its full path in place of `pleno`; on Windows, put a path with spaces in quotes (`"\"C:\\Program Files\\Pleno\\pleno.exe\" run $fileName"`). The Pleno extension's own **Run File** button needs no setup.

## Other editors

Any editor with Language Server Protocol support can use `pleno-lsp`, which talks over standard input and output. Point the editor's LSP client at the `pleno-lsp` command for `.pln` files. The debugger, `pleno-dap`, speaks the Debug Adapter Protocol over standard input and output in the same way.
