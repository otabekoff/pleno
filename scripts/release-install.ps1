# Installs or updates Pleno from a GitHub release: the tools (pleno, plm, pleno-lsp, pleno-dap,
# pleno-repl) into $env:PLENO_HOME\bin (~\.pleno\bin by default) and the VS Code extension from
# the same release. It is the release counterpart of dev-install.ps1, which builds this checkout;
# both install into the same folder, so each replaces what the other installed.
# Run it again to update: it does nothing when the release is already installed.
#
# The tools come from install.ps1 (checksums checked), the extension's .vsix is checked against
# the release's SHA256SUMS.txt. A toolchain that the extension's "Install Toolchain" command
# downloaded is removed, because the extension would use it instead of these tools.
#
# Usage: scripts/release-install.ps1 [-Version v0.4.0] [-Force] [-SkipTools] [-SkipExtension]
#                                    [-KeepExtensionToolchain]
#   -Version  a release tag (default: $env:PLENO_VERSION, else the latest release); an older
#             tag installs that version
#   -Force    install even if that version is already installed
# Needs: for the extension, VS Code's `code` command (without it the extension is skipped).
# PLENO_HOME and PLENO_NO_MODIFY_PATH work as in install.ps1.

param(
    [string]$Version,
    [switch]$Force,
    [switch]$SkipTools,
    [switch]$SkipExtension,
    [switch]$KeepExtensionToolchain
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Repo = "otabekoff/pleno"
$PlenoDir = if ($env:PLENO_HOME) { $env:PLENO_HOME } else { Join-Path $HOME ".pleno" }
$InstallDir = Join-Path $PlenoDir "bin"
$Tools = @("pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl")
$ExtensionId = "pleno.pleno-language"
$ExtensionToolchain = "$env:APPDATA\Code\User\globalStorage\$ExtensionId\toolchain"

function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }
function Warn($message) { Write-Host "warning: $message" -ForegroundColor Yellow }

# Runs VS Code's CLI without the deprecation notices Node prints for code inside it.
function Invoke-Code {
    $saved = $env:NODE_OPTIONS
    $env:NODE_OPTIONS = "--no-deprecation"
    try { & code @args } finally { $env:NODE_OPTIONS = $saved }
}

# The version the installed pleno reports ("pleno 0.4.0" gives "0.4.0"), or $null.
function Get-InstalledVersion {
    $exe = Join-Path $InstallDir "pleno.exe"
    if (-not (Test-Path $exe)) { return $null }
    $line = (& $exe --version 2>$null | Select-Object -First 1)
    if ($line -match "(\d+\.\d+\.\d+\S*)") { return $Matches[1] }
    return $null
}

$Tag = if ($Version) { $Version } elseif ($env:PLENO_VERSION) { $env:PLENO_VERSION } else { $null }
if (-not $Tag) {
    # github.com/<repo>/releases/latest redirects to .../releases/tag/<tag>. Unlike the GitHub
    # API, it has no hourly limit for unauthenticated requests.
    try {
        $response = Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" -Method Head -UseBasicParsing
    } catch {
        throw "Could not reach https://github.com/$Repo/releases/latest: $_"
    }
    # PowerShell 7 and Windows PowerShell 5.1 keep the final address in different places.
    $final = if ($response.BaseResponse.RequestMessage) { $response.BaseResponse.RequestMessage.RequestUri } else { $response.BaseResponse.ResponseUri }
    if ("$final" -notmatch "/releases/tag/([^/?#]+)$") { throw "Could not find the latest release (got $final)" }
    $Tag = $Matches[1]
}
if (-not $Tag.StartsWith("v")) { $Tag = "v$Tag" }
$Wanted = $Tag.Substring(1)
$Base = "https://github.com/$Repo/releases/download/$Tag"

$Installed = Get-InstalledVersion
$From = if ($Installed) { $Installed } else { "not installed" }
Step "Pleno $Wanted from the release $Tag (installed in ${InstallDir}: $From)"

if (-not $SkipTools) {
    if ($Installed -eq $Wanted -and -not $Force) {
        Step "The tools are already $Wanted (pass -Force to install them again)"
    } else {
        Step $(if ($Installed) { "Replacing the tools $Installed with $Wanted" } else { "Installing the tools" })
        $saved = $env:PLENO_VERSION
        $env:PLENO_VERSION = $Tag
        try { & (Join-Path $PSScriptRoot "install.ps1") } finally { $env:PLENO_VERSION = $saved }
    }

    # Another copy found first on PATH would hide this one: one warning per folder, with the
    # command that removes it when the folder belongs to a package manager.
    $shadowing = @{}
    foreach ($tool in $Tools) {
        $found = Get-Command $tool -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found -and ($found.Source -ne (Join-Path $InstallDir "$tool.exe"))) {
            $shadowing[(Split-Path $found.Source)] += @($tool)
        }
    }
    foreach ($dir in $shadowing.Keys) {
        $how = switch -Wildcard ($dir) {
            "*\scoop\*" { "scoop uninstall pleno" }
            "*\WinGet\*" { "winget uninstall Pleno.Pleno" }
            "*\.cargo\*" { "cargo uninstall pleno-cli pleno-lsp pleno-dap pleno-repl" }
            "*\chocolatey\*" { "choco uninstall pleno" }
            default { "remove them from that folder, or put $InstallDir first on PATH" }
        }
        Warn "$($shadowing[$dir] -join ', ') in $dir come before $InstallDir on PATH, so that copy runs instead; to use this one: $how"
    }

    # The extension prefers a toolchain it downloaded itself over PATH.
    if (Test-Path $ExtensionToolchain) {
        if ($KeepExtensionToolchain) {
            Warn "the extension uses its own toolchain in $ExtensionToolchain instead of $InstallDir"
        } else {
            Step "Removing the toolchain the extension downloaded, so it uses these tools"
            # Its language server may be running; the folder can still be renamed.
            Rename-Item $ExtensionToolchain "toolchain.old.$([DateTime]::Now.Ticks)"
        }
    }
    Get-ChildItem (Split-Path $ExtensionToolchain) -Filter "toolchain.old.*" -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { Remove-Item $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
}

$ExtensionDone = $false
if (-not $SkipExtension) {
    if (-not (Get-Command code -ErrorAction SilentlyContinue)) {
        Warn "VS Code's 'code' command was not found, so the extension was not installed (pass -SkipExtension to leave it out)"
    } else {
        $current = Invoke-Code --list-extensions --show-versions |
            Where-Object { $_ -like "$ExtensionId@*" } | Select-Object -First 1
        $currentVersion = if ($current) { $current.Split("@")[1] } else { $null }
        if ($currentVersion -eq $Wanted -and -not $Force) {
            Step "The VS Code extension is already $Wanted"
        } else {
            $vsixName = "pleno-language-$Tag.vsix"
            $work = Join-Path ([System.IO.Path]::GetTempPath()) "pleno-release-$([DateTime]::Now.Ticks)"
            New-Item -ItemType Directory -Force -Path $work | Out-Null
            try {
                Step "Downloading the VS Code extension $vsixName"
                Invoke-WebRequest -Uri "$Base/$vsixName" -OutFile "$work\$vsixName"
                Invoke-WebRequest -Uri "$Base/SHA256SUMS.txt" -OutFile "$work\SHA256SUMS.txt"
                # Lines are "<hash>  ./<file>" (or "*<file>" from sha256sum -b).
                $line = Get-Content "$work\SHA256SUMS.txt" |
                    Where-Object { ((($_ -split '\s+')[1]) -replace '^(\./|\*)', '') -eq $vsixName } |
                    Select-Object -First 1
                if (-not $line) { throw "$vsixName is not in the release's SHA256SUMS.txt" }
                $expected = ($line -split '\s+')[0]
                $actual = (Get-FileHash "$work\$vsixName" -Algorithm SHA256).Hash
                if ($actual -ne $expected) {
                    throw "Checksum mismatch for $vsixName (expected $expected, got $actual)"
                }
                Step $(if ($currentVersion) { "Replacing the extension $currentVersion with $Wanted" } else { "Installing the extension" })
                Invoke-Code --install-extension "$work\$vsixName" --force
                if ($LASTEXITCODE -ne 0) { throw "code --install-extension failed" }
                $ExtensionDone = $true
            } finally {
                Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
    }
}

Step "Installed"
if (-not $SkipTools) {
    foreach ($tool in @("pleno", "pleno-lsp")) {
        Write-Host ("  {0,-10} {1}" -f $tool, (& (Join-Path $InstallDir "$tool.exe") --version))
    }
}
if ($ExtensionDone) {
    Write-Host "  Reload VS Code (Developer: Reload Window) to use the new extension and language server."
}
