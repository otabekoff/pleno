# Pleno Installer for Windows
# Downloads a release from GitHub, checks its SHA-256 checksum and installs
# pleno, plm, pleno-lsp, pleno-dap and pleno-repl into $env:PLENO_HOME\bin
# (~\.pleno\bin by default), then adds that folder to your user PATH.
#
#   irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.ps1 | iex
#
# Environment variables:
#   PLENO_HOME            where to install (default: ~\.pleno); plm keeps its
#                         package cache there too
#   PLENO_VERSION         a release tag such as v0.3.0 (default: the latest release)
#   PLENO_NO_MODIFY_PATH  set to 1 to leave your user PATH alone

# A script block, so that `irm ... | iex` does not change the caller's
# $ErrorActionPreference or leave variables behind.
& {
    $ErrorActionPreference = "Stop"
    $ProgressPreference = "SilentlyContinue"

    # Windows on ARM runs the x86_64 build through emulation.
    switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64" { $Arch = "x86_64" }
        "ARM64" { $Arch = "x86_64"; Write-Host "Windows on ARM: installing the x86_64 build, which runs under emulation." }
        default { throw "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
    }

    $Repo = "otabekoff/pleno"

    $Tag = $env:PLENO_VERSION
    if (-not $Tag) {
        try {
            $Tag = (Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest").tag_name
        } catch {
            throw "Failed to fetch the latest release version: $_"
        }
    }
    if (-not $Tag.StartsWith("v")) { $Tag = "v$Tag" }

    $PlenoDir = if ($env:PLENO_HOME) { $env:PLENO_HOME } else { Join-Path $HOME ".pleno" }
    $InstallDir = Join-Path $PlenoDir "bin"
    $AssetName = "pleno-$Tag-$Arch-pc-windows-msvc.zip"
    $DownloadUrl = "https://github.com/$Repo/releases/download/$Tag/$AssetName"
    $ZipPath = Join-Path ([System.IO.Path]::GetTempPath()) $AssetName
    $ExtractDir = Join-Path ([System.IO.Path]::GetTempPath()) "pleno-install-$([DateTime]::Now.Ticks)"

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    # Files a previous update could not delete because they were running.
    Get-ChildItem $InstallDir -Filter "*.old.exe" -ErrorAction SilentlyContinue |
        ForEach-Object { Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue }

    Write-Host "Downloading Pleno $Tag..."
    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath
    } catch {
        throw "Could not download ${DownloadUrl}: $_"
    }

    try {
        # Verify checksum against the published .sha256 file
        Write-Host "Verifying checksum..."
        Invoke-WebRequest -Uri "$DownloadUrl.sha256" -OutFile "$ZipPath.sha256"
        $ExpectedHash = ((Get-Content "$ZipPath.sha256" -Raw).Trim() -split '\s+')[0]
        Remove-Item "$ZipPath.sha256"
        $ActualHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash
        if ($ActualHash -ne $ExpectedHash) {
            throw "Checksum mismatch for $AssetName (expected $ExpectedHash, got $ActualHash)"
        }

        Write-Host "Installing in $InstallDir..."
        Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force
        foreach ($file in Get-ChildItem $ExtractDir -File) {
            $target = Join-Path $InstallDir $file.Name
            if (Test-Path $target) {
                # A running program (such as the language server VS Code started) cannot be
                # overwritten or deleted on Windows, but it can be renamed; it keeps running
                # from the renamed file, and the next update deletes it.
                try {
                    Remove-Item $target -Force
                } catch {
                    Move-Item $target (Join-Path $InstallDir "$($file.BaseName).$([DateTime]::Now.Ticks).old.exe") -Force
                }
            }
            Move-Item $file.FullName $target
        }
    } finally {
        Remove-Item $ZipPath -ErrorAction SilentlyContinue
        Remove-Item $ExtractDir -Recurse -Force -ErrorAction SilentlyContinue
    }

    # Every tool must now be the new version; a mix of versions would fail in confusing ways.
    $Expected = $Tag.TrimStart("v")
    foreach ($tool in @("pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl")) {
        $exe = Join-Path $InstallDir "$tool.exe"
        if (-not (Test-Path $exe)) { continue }
        # Collect the output before taking the first line: in Windows PowerShell 5.1, `| Select-Object
        # -First 1` on a program's output stops the program as soon as the first line arrives.
        $reported = @(& $exe --version 2>$null)[0]
        if ($reported -notmatch "(^|\s)v?$([regex]::Escape($Expected))(\s|$)") {
            throw "$exe reports '$reported', not $Expected; close the programs that use it (such as VS Code) and run the installer again"
        }
    }

    if ($env:PLENO_NO_MODIFY_PATH -eq "1") {
        Write-Host "Add $InstallDir to your PATH to use it."
    } else {
        $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $Entries = @($UserPath -split ";" | Where-Object { $_ })
        if ($Entries -notcontains $InstallDir) {
            Write-Host "Adding $InstallDir to your user PATH..."
            [Environment]::SetEnvironmentVariable("Path", (($Entries + $InstallDir) -join ";"), "User")
            Write-Host "PATH updated. Restart your terminal."
        } else {
            Write-Host "$InstallDir is already in your PATH."
        }
    }

    Write-Host "Pleno $Tag installed in $InstallDir. Try: pleno --version"
}
