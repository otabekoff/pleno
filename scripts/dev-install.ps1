# Installs Pleno from this checkout: builds the release binaries (pleno, plm,
# pleno-lsp, pleno-dap, pleno-repl) into $env:PLENO_HOME\bin (~\.pleno\bin by
# default), the same folder the release installer uses, and installs the VS Code
# extension from ide/vscode.
# Run it again after pulling to update.
#
# A toolchain that the extension's "Install Toolchain" command downloaded is
# removed, because the extension would use it instead of this build.
#
# Usage: scripts/dev-install.ps1 [-SkipTools] [-SkipExtension] [-KeepExtensionToolchain]
# Needs: Rust (cargo); for the extension also Node.js (npm) and the `code` command.

param(
    [switch]$SkipTools,
    [switch]$SkipExtension,
    [switch]$KeepExtensionToolchain
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$PlenoDir = if ($env:PLENO_HOME) { $env:PLENO_HOME } else { Join-Path $HOME ".pleno" }
$InstallDir = Join-Path $PlenoDir "bin"
$Tools = @("pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl")
$ExtensionToolchain = "$env:APPDATA\Code\User\globalStorage\pleno.pleno-language\toolchain"

function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }
function Warn($message) { Write-Host "warning: $message" -ForegroundColor Yellow }

function Require($command, $hint) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "'$command' was not found. $hint"
    }
}

# Runs a Node.js based tool without the deprecation notices Node prints for
# code inside vsce and VS Code's CLI, which this script cannot change.
function Invoke-Quiet([scriptblock]$command) {
    $saved = $env:NODE_OPTIONS
    $env:NODE_OPTIONS = "--no-deprecation"
    try { & $command } finally { $env:NODE_OPTIONS = $saved }
}

# Two runs at once would build into the same folders and break each other.
New-Item -ItemType Directory -Force -Path "$Root\target" | Out-Null
try {
    $lock = [System.IO.File]::Open("$Root\target\.dev-install.lock", "OpenOrCreate", "ReadWrite", "None")
} catch {
    throw "another dev-install is running from $Root; wait for it to finish"
}

try {
    $commit = (git -C $Root rev-parse --short HEAD 2>$null)
    $branch = (git -C $Root rev-parse --abbrev-ref HEAD 2>$null)
    Step "Installing Pleno from $Root ($branch, $commit)"

    if (-not $SkipTools) {
        Require "cargo" "Install Rust from https://rustup.rs"
        Step "Building release binaries"
        cargo build --release --locked --manifest-path "$Root\Cargo.toml" `
            -p pleno-cli -p pleno-lsp -p pleno-dap -p pleno-repl
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        # Copies left from an earlier update, once nothing runs them any more.
        Get-ChildItem $InstallDir -Filter "*.old.exe" -ErrorAction SilentlyContinue |
            ForEach-Object { Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue }

        Step "Copying binaries to $InstallDir"
        foreach ($tool in $Tools) {
            $source = "$Root\target\release\$tool.exe"
            $target = "$InstallDir\$tool.exe"
            if (-not (Test-Path $source)) { throw "missing $source" }
            if (Test-Path $target) {
                # A running program (the language server inside VS Code) cannot be
                # overwritten on Windows, but it can be renamed.
                Move-Item $target "$InstallDir\$tool.$([DateTime]::Now.Ticks).old.exe" -Force
            }
            Copy-Item $source $target
        }

        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if (($userPath -split ";") -notcontains $InstallDir) {
            Step "Adding $InstallDir to your user PATH"
            [Environment]::SetEnvironmentVariable("Path", "$InstallDir;$userPath", "User")
        }
        $env:Path = "$InstallDir;$env:Path"

        # Another copy found first on PATH would hide this one.
        foreach ($tool in $Tools) {
            $other = Get-Command $tool -All -ErrorAction SilentlyContinue |
                Where-Object { $_.Source -and (Split-Path $_.Source) -ne $InstallDir }
            foreach ($copy in $other) {
                Warn "another $tool is at $($copy.Source); remove it (for cargo installs: cargo uninstall pleno-cli pleno-lsp pleno-dap pleno-repl) so it cannot shadow this one"
            }
        }

        # The extension prefers a toolchain it downloaded itself over PATH.
        if (Test-Path $ExtensionToolchain) {
            if ($KeepExtensionToolchain) {
                Warn "the extension uses its own toolchain in $ExtensionToolchain instead of $InstallDir"
            } else {
                Step "Removing the toolchain the extension downloaded, so it uses this build"
                # Its language server may be running; the folder can still be renamed.
                Rename-Item $ExtensionToolchain "toolchain.old.$([DateTime]::Now.Ticks)"
            }
        }
        Get-ChildItem (Split-Path $ExtensionToolchain) -Filter "toolchain.old.*" -Directory -ErrorAction SilentlyContinue |
            ForEach-Object { Remove-Item $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
    }

    if (-not $SkipExtension) {
        Require "npm" "Install Node.js from https://nodejs.org, or pass -SkipExtension"
        Require "code" "Install VS Code and its 'code' command, or pass -SkipExtension"
        $vsix = Join-Path $env:TEMP "pleno-language-dev.vsix"
        Step "Building the VS Code extension"
        Push-Location "$Root\ide\vscode"
        try {
            npm ci --no-audit --no-fund
            if ($LASTEXITCODE -ne 0) { throw "npm ci failed" }
            # vsce builds the extension itself (the `vscode:prepublish` script).
            Invoke-Quiet { npx --yes @vscode/vsce@3.2.1 package --no-dependencies --out $vsix }
            if ($LASTEXITCODE -ne 0) { throw "vsce package failed" }
        } finally {
            Pop-Location
        }
        Step "Installing the extension"
        Invoke-Quiet { code --install-extension $vsix --force }
        if ($LASTEXITCODE -ne 0) { throw "code --install-extension failed" }
        Remove-Item $vsix -ErrorAction SilentlyContinue
    }

    Step "Installed"
    if (-not $SkipTools) {
        foreach ($tool in @("pleno", "pleno-lsp")) {
            Write-Host ("  {0,-10} {1}" -f $tool, (& "$InstallDir\$tool.exe" --version))
        }
    }
    if (-not $SkipExtension) {
        Write-Host "  Reload VS Code (Developer: Reload Window) to use the new extension and language server."
    }
    Write-Host "  Open a new terminal for the PATH change to take effect."
} finally {
    $lock.Close()
}
