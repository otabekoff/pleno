# Installation

Pleno ships as five programs: `pleno` (run, check, format, test), `plm` (packages), `pleno-lsp` (language server), `pleno-dap` (debugger) and `pleno-repl`.

## With VS Code

Install the [VS Code extension](./editor), then run **Pleno: Install or Update Toolchain** from the Command Palette. It downloads the latest release for your system, checks its SHA-256 checksum, and makes the tools available in VS Code's terminals.

## With a package manager

::: code-group

```sh [Homebrew (macOS, Linux)]
brew install otabekoff/tap/pleno
```

```powershell [Scoop (Windows)]
scoop bucket add pleno https://github.com/otabekoff/scoop-bucket
scoop install pleno
```

:::

Both install the release archives from GitHub, with their checksums, and are updated with each release. `brew upgrade pleno` and `scoop update pleno` get a newer version. WinGet, Chocolatey and the AUR are not available yet.

## With the install script

::: code-group

```sh [Linux and macOS]
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh
```

```powershell [Windows]
irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.ps1 | iex
```

:::

The script downloads the latest release from GitHub into `~/.pleno/bin` (Windows: `%USERPROFILE%\.pleno\bin`) and adds that folder to your `PATH`. Open a new terminal afterwards.

## By hand

1. Download the archive for your platform from the [releases page](https://github.com/otabekoff/pleno/releases):

   | Platform | Archive |
   | --- | --- |
   | Windows (x64) | `pleno-vX.Y.Z-x86_64-pc-windows-msvc.zip` |
   | Linux (x64) | `pleno-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |
   | Linux (ARM64) | `pleno-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz` |
   | Linux (x86, 32-bit) | `pleno-vX.Y.Z-i686-unknown-linux-gnu.tar.gz` |
   | macOS (Intel) | `pleno-vX.Y.Z-x86_64-apple-darwin.tar.gz` |
   | macOS (Apple silicon) | `pleno-vX.Y.Z-aarch64-apple-darwin.tar.gz` |

   The Linux programs need glibc 2.17 or newer, so they run on any distribution from 2014 on (CentOS 7, Ubuntu 14.04, Debian 8 and later). They do not run on musl distributions such as Alpine.

2. Check it against the `.sha256` file next to it.
3. Extract it and put the programs in a folder on your `PATH`.

## From source

With [Rust](https://rustup.rs) installed:

```sh
git clone https://github.com/otabekoff/pleno.git
cd pleno
cargo install --path crates/pleno-cli   # pleno and plm
cargo install --path crates/pleno-lsp
cargo install --path crates/pleno-dap
cargo install --path crates/pleno-repl
```

## Check the installation

```sh
pleno --version
pleno --help
```

## Uninstall

::: code-group

```sh [Linux and macOS]
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/uninstall.sh | sh
```

```powershell [Windows]
irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/uninstall.ps1 | iex
```

:::

Next: [write your first program](./getting-started).
