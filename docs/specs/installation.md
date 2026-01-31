# Installation

There are multiple ways to install Pleno. The recommended way is to use the automated installation scripts, which will download the latest release and set up your environment.

## Automated Install (Recommended)

### Linux & macOS

Run the following command in your terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh
```

This script will:
1.  Detect your OS and architecture.
2.  Download the latest `pleno` release from GitHub.
3.  Install it to `~/.pleno/bin`.
4.  Prompt you to add `~/.pleno/bin` to your `PATH`.

### Windows (PowerShell)

Run the following command in PowerShell:

```powershell
iwr https://raw.githubusercontent.com/otabekoff/pleno/main/tasks/install.ps1 -useb | iex
```

This script will:
1.  Detect your architecture.
2.  Download the latest `pleno` release from GitHub.
3.  Install it to `$HOME\.pleno\bin`.
4.  Automatically add it to your user `PATH` (requires a terminal restart).

## Manual Installation

If you prefer to install manually:

1.  Go to the [Releases Page](https://github.com/otabekoff/pleno/releases).
2.  Download the archive for your platform:
    *   **Windows**: `pleno-vX.Y.Z-x86_64-pc-windows-msvc.zip`
    *   **Linux**: `pleno-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
    *   **macOS (Intel)**: `pleno-vX.Y.Z-x86_64-apple-darwin.tar.gz`
    *   **macOS (Apple Silicon)**: `pleno-vX.Y.Z-aarch64-apple-darwin.tar.gz`
3.  Extract the archive.
4.  Move the `pleno` binary to a folder in your system `PATH` (e.g., `/usr/local/bin` on Unix).

## Verifying Installation

Once installed, verify that Pleno is available by checking the version:

```bash
pleno --version
```

You can also view the help menu to see available commands:

```bash
pleno --help
```
