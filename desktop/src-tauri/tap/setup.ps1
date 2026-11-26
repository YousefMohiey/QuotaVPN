# Create or adopt the QuotaVPN layer-2 adapter, name it, and record its GUID
# where the app looks for it. Runs from the installer, elevated.

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$inf = Join-Path $here 'amd64\OemVista.inf'
$devcon = Join-Path $here 'amd64\devcon.exe'
if (-not (Test-Path -LiteralPath $inf) -or -not (Test-Path -LiteralPath $devcon)) { exit 1 }

$tap = Get-NetAdapter -ErrorAction SilentlyContinue |
  Where-Object { $_.InterfaceDescription -like '*TAP-Windows*' } |
  Sort-Object ifIndex |
  Select-Object -First 1

if (-not $tap) {
  & $devcon install $inf TAP0901 | Out-Null
  Start-Sleep -Seconds 1
  $tap = Get-NetAdapter -ErrorAction SilentlyContinue |
    Where-Object { $_.InterfaceDescription -like '*TAP-Windows*' } |
    Sort-Object ifIndex |
    Select-Object -First 1
}
if (-not $tap) { exit 1 }

if ($tap.Name -ne 'QuotaSpaceLAN') {
  $taken = Get-NetAdapter -Name 'QuotaSpaceLAN' -ErrorAction SilentlyContinue
  if (-not $taken) {
    Rename-NetAdapter -Name $tap.Name -NewName 'QuotaSpaceLAN'
  }
}

$tap = Get-NetAdapter -Name 'QuotaSpaceLAN' -ErrorAction SilentlyContinue
if (-not $tap) {
  $tap = Get-NetAdapter -ErrorAction SilentlyContinue |
    Where-Object { $_.InterfaceDescription -like '*TAP-Windows*' } |
    Select-Object -First 1
}
if (-not $tap) { exit 1 }

$dir = Join-Path $env:APPDATA 'quotavpn'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
[IO.File]::WriteAllText((Join-Path $dir 'spaces_tap.txt'), "$($tap.InterfaceGuid)")
exit 0
