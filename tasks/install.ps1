# Pleno Windows Installer Script

$ErrorActionPreference = "Stop"

Write-Host "Installing Pleno..." -ForegroundColor Cyan

# 1. Determine Installation Directory
$InstallDir = "$env:USERPROFILE\.pleno\bin"
if (-not (Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Write-Host "Created installation directory: $InstallDir" -ForegroundColor Green
}

# 2. Build Binaries
Write-Host "Building release binaries..." -ForegroundColor Yellow
cargo build --release --workspace

# 3. Determine Build Output Directory
try {
    $Metadata = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $TargetDir = $Metadata.target_directory
    Write-Host "Detected target directory: $TargetDir" -ForegroundColor Gray
} catch {
    Write-Warning "Could not detect target directory via cargo metadata. Defaulting to 'target'."
    $TargetDir = Join-Path (Get-Location) "target"
}

# 4. Copy Binaries
$SourceDir = "$TargetDir\release"
$Binaries = @("pleno-cli.exe", "pleno-repl.exe", "pleno-lsp.exe")

foreach ($Binary in $Binaries) {
    if (Test-Path "$SourceDir\$Binary") {
        Copy-Item -Path "$SourceDir\$Binary" -Destination "$InstallDir\$Binary" -Force
        Write-Host "Installed $Binary" -ForegroundColor Green
        
        # Renaissance: Rename pleno-cli to pleno
        if ($Binary -eq "pleno-cli.exe") {
            Copy-Item -Path "$InstallDir\$Binary" -Destination "$InstallDir\pleno.exe" -Force
            Write-Host "Aliased pleno-cli.exe to pleno.exe" -ForegroundColor Green
        }
    } else {
        Write-Warning "Binary not found: $Binary (Skipping)"
    }
}

# 4. Update PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host "PATH updated. Please restart your terminal/IDE for changes to take effect." -ForegroundColor Magenta
} else {
    Write-Host "PATH already configured." -ForegroundColor Green
}

Write-Host "Installation Complete!" -ForegroundColor Cyan
Write-Host "Binaries located in: $InstallDir"
