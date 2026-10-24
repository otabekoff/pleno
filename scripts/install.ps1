$ErrorActionPreference = "Stop"

# Detect Architecture
if ($env:PROCESSOR_ARCHITECTURE -eq "AMD64") {
    $Arch = "x86_64"
} else {
    Write-Error "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE"
}

$Repo = "otabekoff/pleno"

# Get latest tag
try {
    $LatestRelease = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Tag = $LatestRelease.tag_name
} catch {
    Write-Error "Failed to fetch latest release version."
}

$AssetName = "pleno-$Tag-$Arch-pc-windows-msvc.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$Tag/$AssetName"
$InstallDir = "$HOME\.pleno\bin"
$ZipPath = "$env:TEMP\$AssetName"

# Create install directory
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

Write-Host "Downloading Pleno $Tag..."
Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath

# Verify checksum against the published .sha256 file
Write-Host "Verifying checksum..."
Invoke-WebRequest -Uri "$DownloadUrl.sha256" -OutFile "$ZipPath.sha256"
$ExpectedHash = ((Get-Content "$ZipPath.sha256" -Raw).Trim() -split '\s+')[0]
Remove-Item "$ZipPath.sha256"
$ActualHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash
if ($ActualHash -ne $ExpectedHash) {
    Remove-Item $ZipPath
    Write-Error "Checksum mismatch for $AssetName (expected $ExpectedHash, got $ActualHash)"
}

Write-Host "Extracting to $InstallDir..."
Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force

# Cleanup
Remove-Item $ZipPath

# Path setup
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    Write-Host "PATH updated. Please restart your terminal."
} else {
    Write-Host "Pleno is already in your PATH."
}

Write-Host "Pleno installed successfully!"
