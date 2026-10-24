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
- **Run** and **Debug** buttons for `.pln` files; press `F5` to debug with breakpoints.
- Snippets such as `fn`, `class`, `match` and `tryf`.

**Pleno: Show Pleno Status** shows which `pleno`, `pleno-lsp` and `pleno-dap` the extension uses. To use specific builds, set `pleno.cli.path`, `pleno.lsp.path` and `pleno.dap.path`.

## Other editors

Any editor with Language Server Protocol support can use `pleno-lsp`, which talks over standard input and output. Point the editor's LSP client at the `pleno-lsp` command for `.pln` files. The debugger, `pleno-dap`, speaks the Debug Adapter Protocol over standard input and output in the same way.
