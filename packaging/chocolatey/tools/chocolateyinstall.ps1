$ErrorActionPreference = 'Stop'

$packageName = 'pleno'
$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

$url64 = 'https://github.com/otabekoff/pleno/releases/download/v$version$/pleno-v$version$-x86_64-pc-windows-msvc.zip'
$checksum64 = '$checksum64$'

$packageArgs = @{
  packageName    = $packageName
  unzipLocation  = $toolsDir
  url64bit       = $url64
  checksum64     = $checksum64
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

# Add to PATH
$binPath = Join-Path $toolsDir 'pleno.exe'
Install-BinFile -Name 'pleno' -Path $binPath

$plmPath = Join-Path $toolsDir 'plm.exe'
Install-BinFile -Name 'plm' -Path $plmPath

Write-Host "Pleno has been installed successfully!" -ForegroundColor Green
Write-Host "Run 'pleno --version' to verify the installation." -ForegroundColor Cyan
