# Pleno Uninstaller
# Reverses scripts/install.ps1: removes $env:PLENO_HOME (~\.pleno by default),
# with the installed tools and plm's package cache, and its entry in the user PATH.

& {
    $ErrorActionPreference = "Stop"

    $PlenoDir = if ($env:PLENO_HOME) { $env:PLENO_HOME } else { Join-Path $HOME ".pleno" }
    $InstallDir = Join-Path $PlenoDir "bin"

    if (Test-Path $PlenoDir) {
        Remove-Item -Recurse -Force $PlenoDir
        Write-Host "Removed $PlenoDir"
    } else {
        Write-Host "$PlenoDir does not exist; nothing to remove"
    }

    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $Entries = $UserPath -split ";" | Where-Object { $_ -and ($_ -ne $InstallDir) }
    $NewPath = $Entries -join ";"
    if ($NewPath -ne $UserPath) {
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
        Write-Host "Removed $InstallDir from your user PATH"
    }

    Write-Host "Pleno uninstalled. Restart your terminal to update PATH."
}
