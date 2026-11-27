param(
  [string]$AppDir = "desktop\src-tauri\target\release",
  [string]$Installer = "",
  [string]$Thumbprint = $env:QC_SIGN_THUMBPRINT,
  [string]$Pfx = "",
  [string]$PfxPassword = "",
  [switch]$IncludeSigned
)

# Sign every payload binary and, when given, the installer.
#
# wintun.dll ships signed by WireGuard LLC and is left alone. Everything else
# that can be launched or loaded is signed, because Smart App Control checks
# each load, not just the first one.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$signer = Join-Path $PSScriptRoot "sign-file.ps1"

if (-not $Thumbprint -and -not $Pfx) {
  throw "no certificate: pass -Thumbprint or -Pfx, or run tools/make-test-cert.ps1 first"
}

$appPath = if ([IO.Path]::IsPathRooted($AppDir)) { $AppDir } else { Join-Path $root $AppDir }
if (-not (Test-Path -LiteralPath $appPath)) { throw "missing app directory: $appPath" }

$targets = @()
$targets += Get-ChildItem $appPath -Filter *.exe -File -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -notin @("QuotaVPN-mcp.exe", "QuotaVPN-normal-backup.exe") }
$targets += Get-ChildItem $appPath -Filter *.dll -File -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -ne "wintun.dll" }
# The bundled LAN engine lives in its own folder and is launched as the
# QuotaSpace process, so it carries the same publisher as the app. The old
# name is kept for a tree that has not been renamed yet.
foreach ($name in @("QuotaSpace.exe", "openrad.exe")) {
  $engine = Join-Path $appPath "openrad\$name"
  if (Test-Path -LiteralPath $engine) { $targets += Get-Item -LiteralPath $engine }
}

if ($Installer) {
  $installerPath = if ([IO.Path]::IsPathRooted($Installer)) { $Installer } else { Join-Path $root $Installer }
  if (-not (Test-Path -LiteralPath $installerPath)) { throw "missing installer: $installerPath" }
  $targets += Get-Item -LiteralPath $installerPath
}

if (-not $targets) { throw "nothing to sign under $appPath" }

if (-not $IncludeSigned) {
  $targets = $targets | Where-Object {
    (Get-AuthenticodeSignature -LiteralPath $_.FullName).Status -ne "Valid"
  }
}

if (-not $targets) {
  Write-Output "everything is already signed"
  return
}

$list = $targets | ForEach-Object { $_.FullName }
$signArgs = @{ Files = $list }
if ($Pfx) { $signArgs["Pfx"] = $Pfx } else { $signArgs["Thumbprint"] = $Thumbprint }
if ($PfxPassword) { $signArgs["PfxPassword"] = $PfxPassword }

& $signer @signArgs

Write-Output ""
Write-Output "result:"
foreach ($t in $targets) {
  $s = Get-AuthenticodeSignature -LiteralPath $t.FullName
  Write-Output ("  {0} {1} {2}" -f $s.Status, $s.SignerCertificate.Subject, $t.Name)
  if ($s.Status -ne "Valid") { throw "signature not valid on $($t.Name)" }
}
