param(
  [Parameter(Mandatory = $true)][string[]]$Files,
  [string]$Thumbprint = $env:QC_SIGN_THUMBPRINT,
  [string]$Pfx = "",
  [string]$PfxPassword = "",
  [string]$Timestamp = "http://timestamp.digicert.com"
)

# Sign Windows binaries with Authenticode and prove the result.
#
# Two ways in, both accepted by signtool:
#   -Thumbprint  the certificate sits in the Windows store, which is how
#                SimplySign and the other cloud HSM clients present it.
#   -Pfx         a file held private key (test certificates, mostly).
#
# Every file is signed with SHA-256 and timestamped, because an unsigned
# timestamp means the signature dies with the certificate and installed copies
# start warning again.

$ErrorActionPreference = "Stop"

$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -like "*x64*" } |
  Sort-Object FullName -Descending |
  Select-Object -First 1
if (-not $signtool) {
  throw "signtool.exe not found. Install the Windows SDK (Windows Kits 10) or the signing tools."
}

foreach ($f in $Files) {
  if (-not (Test-Path -LiteralPath $f)) { throw "missing file: $f" }
  Write-Output "signing $f"
  $args = @("sign", "/fd", "sha256", "/td", "sha256", "/tr", $Timestamp, "/v")
  if ($Pfx) {
    $args += @("/f", $Pfx)
    if ($PfxPassword) { $args += @("/p", $PfxPassword) }
  } elseif ($Thumbprint) {
    $args += @("/sha1", $Thumbprint)
  } else {
    throw "no certificate: pass -Thumbprint or -Pfx, or set QC_SIGN_THUMBPRINT"
  }
  $args += $f
  & $signtool.FullName @args
  if ($LASTEXITCODE -ne 0) { throw "signtool failed on $f" }

  & $signtool.FullName verify /pa /v $f
  if ($LASTEXITCODE -ne 0) { throw "signature did not verify on $f" }
  Write-Output "ok $f"
}
