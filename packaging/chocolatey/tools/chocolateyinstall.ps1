$ErrorActionPreference = 'Stop'

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

# Chocolatey puts a shim on the PATH for every .exe in the package folder:
# pleno, plm, pleno-lsp, pleno-dap and pleno-repl.
$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  unzipLocation  = $toolsDir
  url64bit       = 'https://github.com/otabekoff/pleno/releases/download/v0.4.0/pleno-v0.4.0-x86_64-pc-windows-msvc.zip'
  checksum64     = 'db104e5e2ca2d4534de5752c8e33b61dda1ec111725ab41ecf1f5f3dcae4bb07'
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs
