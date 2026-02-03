$ErrorActionPreference = 'Stop'

$packageName = 'pleno'
$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

Uninstall-BinFile -Name 'pleno'
Uninstall-BinFile -Name 'plm'

Write-Host "Pleno has been uninstalled successfully!" -ForegroundColor Green
