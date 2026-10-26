$ErrorActionPreference = 'Stop'

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

# Chocolatey puts a shim on the PATH for every .exe in the package folder:
# pleno, plm, pleno-lsp, pleno-dap and pleno-repl.
$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  unzipLocation  = $toolsDir
  url64bit       = 'https://github.com/otabekoff/pleno/releases/download/v0.3.0/pleno-v0.3.0-x86_64-pc-windows-msvc.zip'
  checksum64     = '792caa68649a668a9a24a05b99683dd41e693e80de10b8eb4c18673f6cc6329d'
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs
