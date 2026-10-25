# Install the toolchain

**Pleno: Install or Update Toolchain** downloads the latest release for your system from
[GitHub](https://github.com/otabekoff/pleno/releases), checks its SHA-256 checksum and installs
`pleno`, `plm`, `pleno-lsp`, `pleno-dap` and `pleno-repl` into the extension's storage. VS Code's
integrated terminals get them on `PATH`. Run the command again later to update.

To install system-wide instead, run the install script:

```sh
# Linux and macOS
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh
```

```powershell
# Windows
irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.ps1 | iex
```

The extension looks for each tool in this order:

1. The `pleno.cli.path`, `pleno.lsp.path` and `pleno.dap.path` settings.
2. The toolchain installed by the extension.
3. `PATH`, then `~/.pleno/bin`, where the install script puts it.
4. `target/release` or `target/debug`, when you work on Pleno itself.

**Pleno: Show Status**, or the **Pleno** item in the status bar, shows which version of each
tool was found and where.
