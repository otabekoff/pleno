# Set Up the Pleno Toolchain

The extension needs the Pleno toolchain: `pleno` (run, check, format), `pleno-lsp` (language server) and `pleno-dap` (debugger).

### Install from VS Code

Run **Pleno: Install or Update Toolchain**. It downloads the latest GitHub release for your system, verifies its checksum, and installs it into the extension's storage. Run it again later to update.

[Install or Update](command:pleno.installToolchain)

### Install system-wide

Linux and macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.ps1 | iex
```

### Where the extension looks

1. The `pleno.cli.path`, `pleno.lsp.path` and `pleno.dap.path` settings.
2. The toolchain installed by the extension.
3. Your PATH.
4. A local build (`target/release` or `target/debug`) when you work on Pleno itself.

[Show Toolchain Status](command:pleno.showStatus)
