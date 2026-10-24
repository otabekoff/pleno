$ErrorActionPreference = 'Stop'

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

# Chocolatey puts a shim on the PATH for every .exe in the package folder:
# pleno, plm, pleno-lsp, pleno-dap and pleno-repl.
$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  unzipLocation  = $toolsDir
  url64bit       = 'https://github.com/otabekoff/pleno/releases/download/v0.2.0/pleno-v0.2.0-x86_64-pc-windows-msvc.zip'
  checksum64     = '2b26d51c972ddb99422442408a4e562e5ac79e5a09e60d852902febb9ad339ba'
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs
