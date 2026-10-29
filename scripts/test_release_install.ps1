# Tests scripts/release-install.ps1 against the real GitHub releases, in a temporary PLENO_HOME
# and APPDATA, with a stand-in for VS Code's `code` command that records what it installs:
#   1. a fresh install gets the latest release's tools and extension, and removes a toolchain
#      the extension downloaded;
#   2. running it again changes nothing;
#   3. -Version v0.3.0 installs that older release, tools and extension;
#   4. running it again updates both to the latest while the old language server is running
#      (Windows cannot overwrite a running .exe; install.ps1 must rename it aside).
#
# Usage: scripts/test_release_install.ps1   (needs network access to github.com)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Root = Split-Path -Parent $PSScriptRoot
$Work = Join-Path ([System.IO.Path]::GetTempPath()) "pleno-release-install-test-$([DateTime]::Now.Ticks)"
$Shim = Join-Path $Work "shim"
$Toolchain = Join-Path $Work "appdata\Code\User\globalStorage\pleno.pleno-language\toolchain"
New-Item -ItemType Directory -Force -Path $Shim, $Toolchain | Out-Null

$saved = @{ PATH = $env:PATH; APPDATA = $env:APPDATA; PLENO_HOME = $env:PLENO_HOME
    PLENO_NO_MODIFY_PATH = $env:PLENO_NO_MODIFY_PATH; PLENO_VERSION = $env:PLENO_VERSION }
$env:APPDATA = Join-Path $Work "appdata"
$env:PLENO_HOME = Join-Path $Work "pleno"
$env:PLENO_NO_MODIFY_PATH = "1"
$env:PLENO_VERSION = $null
$env:PATH = "$Shim;$env:PATH"
$Bin = Join-Path $env:PLENO_HOME "bin"

# The stand-in `code`: --list-extensions prints what it installed, --install-extension checks
# that the .vsix is a real extension (a zip with extension/package.json) and records its version.
@'
param([Parameter(ValueFromRemainingArguments)]$a)
$ErrorActionPreference = "Stop"
$state = Join-Path $PSScriptRoot "installed.txt"
if ($a -contains "--list-extensions") { if (Test-Path $state) { Get-Content $state }; "other.extension@1.0.0"; exit 0 }
if ($a[0] -eq "--install-extension") {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($a[1])
    try {
        $entry = $zip.GetEntry("extension/package.json")
        if (-not $entry) { Write-Error "not a .vsix: $($a[1])" }
        $reader = New-Object System.IO.StreamReader($entry.Open())
        $version = ($reader.ReadToEnd() | ConvertFrom-Json).version
        $reader.Close()
    } finally { $zip.Dispose() }
    "pleno.pleno-language@$version" | Set-Content $state
    Add-Content (Join-Path $PSScriptRoot "log.txt") "install $version"
    exit 0
}
exit 2
'@ | Set-Content (Join-Path $Shim "fake-code.ps1")
"@powershell -NoProfile -ExecutionPolicy Bypass -File `"%~dp0fake-code.ps1`" %*" |
    Set-Content (Join-Path $Shim "code.cmd") -Encoding ascii

$script:failures = 0
function Fail($message) { Write-Host "FAIL: $message" -ForegroundColor Red; $script:failures++ }
function Test-Tools($version, $label) {
    foreach ($tool in @("pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl")) {
        $got = (& (Join-Path $Bin "$tool.exe") --version 2>$null | Select-Object -First 1)
        if ($got -ne "$tool $version") { Fail "${label}: $tool --version is '$got', expected '$tool $version'" }
    }
}
function Test-Extension($version, $label) {
    $got = Get-Content (Join-Path $Shim "installed.txt") -ErrorAction SilentlyContinue
    if ($got -ne "pleno.pleno-language@$version") { Fail "${label}: the extension is '$got', expected $version" }
}
function Get-Installs { @(Get-Content (Join-Path $Shim "log.txt") -ErrorAction SilentlyContinue).Count }
function Invoke-ReleaseInstall {
    $script:out = & (Join-Path $Root "scripts\release-install.ps1") @args *>&1 | Out-String
}

$lsp = $null
try {
    $response = Invoke-WebRequest -Uri "https://github.com/otabekoff/pleno/releases/latest" -Method Head -UseBasicParsing
    $final = if ($response.BaseResponse.RequestMessage) { $response.BaseResponse.RequestMessage.RequestUri } else { $response.BaseResponse.ResponseUri }
    $latest = ("$final" -split "/v")[-1]
    $older = "0.3.0"
    if ($latest -eq $older) { throw "the latest release must be newer than $older for this test" }
    Write-Host "latest release: $latest"

    Invoke-ReleaseInstall
    Test-Tools $latest "fresh install"
    Test-Extension $latest "fresh install"
    if (Test-Path $Toolchain) { Fail "fresh install: the extension's own toolchain was not removed" }
    if ((Get-Installs) -ne 1) { Fail "fresh install: the extension was installed $(Get-Installs) times" }

    Invoke-ReleaseInstall
    if ($out -notmatch "already $([regex]::Escape($latest))") { Fail "second run: no 'already $latest' message" }
    if ((Get-Installs) -ne 1) { Fail "second run: the extension was installed again" }
    Test-Tools $latest "second run"

    Invoke-ReleaseInstall -Version "v$older"
    Test-Tools $older "-Version v$older"
    Test-Extension $older "-Version v$older"

    # The language server keeps running while it is replaced, as when VS Code has it open.
    $lsp = Start-Process -FilePath (Join-Path $Bin "pleno-lsp.exe") -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds 1
    if ($lsp.HasExited) { throw "the language server did not stay running, so step 4 would test nothing" }
    Invoke-ReleaseInstall
    Test-Tools $latest "update with the language server running"
    Test-Extension $latest "update with the language server running"
    if ($lsp.HasExited) { Fail "the running language server stopped during the update" }
} finally {
    if ($lsp -and -not $lsp.HasExited) { Stop-Process -Id $lsp.Id -Force; $lsp.WaitForExit() }
    foreach ($name in $saved.Keys) {
        if ($null -eq $saved[$name]) { Remove-Item "env:$name" -ErrorAction SilentlyContinue }
        else { Set-Item "env:$name" $saved[$name] }
    }
    Remove-Item $Work -Recurse -Force -ErrorAction SilentlyContinue
}

if ($script:failures -ne 0) {
    Write-Host "$($script:failures) check(s) failed" -ForegroundColor Red
    exit 1
}
Write-Host "release-install.ps1: all checks passed"
