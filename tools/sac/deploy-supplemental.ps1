param(
  [string]$Pfx = "C:\Tools\qc-policy.pfx",
  [string]$PasswordFile = "C:\Tools\qc-policy-pass.txt",
  [string]$Certificate = "C:\Tools\qc-policy.cer",
  [string]$Xml = "",
  [switch]$Remove
)

# Deploy or remove the QuotaVPN supplemental App Control policy.
#
# Sequence: fill the policy certificate hash into the template, compile it,
# sign it as a Code Integrity policy, hand it to citool, then read the policy
# list back. Run elevated.
#
# A policy signed by a locally trusted certificate is only authorized after a
# reboot, because Code Integrity reads the trust anchors at boot. The script
# says so when the policy is not authorized yet.
#
# Removal only needs the policy ID.

$ErrorActionPreference = "Stop"
$policyId = "{7C1D2C40-9C2B-4F0E-9E1B-51F0C7A6D001}"
$template = if ($Xml) { $Xml } else { Join-Path $PSScriptRoot "quotavpn-supplemental.xml" }

if ($Remove) {
  & citool.exe -rp $policyId
  if ($LASTEXITCODE -ne 0) { throw "citool could not remove $policyId" }
  Write-Output "removed $policyId"
  return
}

$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -like "*x64*" } |
  Sort-Object FullName -Descending |
  Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found" }
if (-not (Test-Path -LiteralPath $Pfx)) { throw "missing $Pfx, run tools/make-test-cert.ps1 first" }
if (-not (Test-Path -LiteralPath $PasswordFile)) { throw "missing $PasswordFile" }
if (-not (Test-Path -LiteralPath $Certificate)) { throw "missing $Certificate" }

# TBS is the SHA-256 of the whole certificate, uppercase hex.
$cert = New-Object Security.Cryptography.X509Certificates.X509Certificate2 (Resolve-Path $Certificate).Path
$sha = [Security.Cryptography.SHA256]::Create()
$tbs = ($sha.ComputeHash($cert.RawData) | ForEach-Object { $_.ToString("X2") }) -join ""

$work = Join-Path $env:TEMP "qc-supplemental"
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $work | Out-Null
$filled = Join-Path $work "quotavpn-supplemental.xml"
Set-Content -LiteralPath $filled -Value ((Get-Content -LiteralPath $template -Raw) -replace "__POLICY_TBS__", $tbs) -NoNewline

$unsigned = Join-Path $work "quotavpn-supplemental.cip"
$out = Join-Path $work "signed"
New-Item -ItemType Directory -Force -Path $out | Out-Null

ConvertFrom-CIPolicy -XmlFilePath $filled -BinaryFilePath $unsigned
Write-Output "compiled the supplement"

$password = [IO.File]::ReadAllText($PasswordFile).Trim()
# 1.3.6.1.4.1.311.79.1 is the Code Integrity policy content type, not the
# 311.10.3.12 EKU. With the wrong OID citool accepts the file and ignores it.
& $signtool.FullName sign /fd sha256 /f $Pfx /p $password /p7 $out /p7co 1.3.6.1.4.1.311.79.1 /p7ce EmbeddedSignedData $unsigned
if ($LASTEXITCODE -ne 0) { throw "signing the policy failed" }
$signed = Join-Path $work "quotavpn-supplemental-signed.cip"
Copy-Item (Join-Path $out "quotavpn-supplemental.cip.p7") $signed -Force

& citool.exe -up $signed
if ($LASTEXITCODE -ne 0) { throw "citool rejected the policy" }

$entry = & citool.exe -lp | Select-String -Pattern $policyId -SimpleMatch -Context 0,8
if (-not $entry) { throw "policy is not in the list after deployment" }
$authorized = $entry.Context.PostContext -match "Is Authorized: true"
if ($authorized) {
  Write-Output "deployed and authorized $policyId"
} else {
  Write-Output "deployed $policyId, not authorized yet: reboot and check citool -lp"
}
