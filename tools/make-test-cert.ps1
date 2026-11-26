param(
  # Text Windows shows in the UAC prompt. It is cosmetic: the certificate is
  # self-signed, so the chain ends at a root only this machine trusts.
  [string]$Publisher = "QuotaVPN",
  [string]$PfxPath = "C:\Tools\qc-testsign.pfx",
  [string]$CerPath = "C:\Tools\qc-testsign.cer",
  [string]$PasswordPath = "C:\Tools\qc-testsign-pass.txt",
  [switch]$Force
)

# Create (or reuse) a local code signing certificate, export it, and trust its
# root on this machine.
#
# What this buys:
#   - UAC shows the publisher name instead of "Unknown publisher"
#   - file properties show a signature and the hashes match after signing
#   - antivirus heuristics that dislike unsigned binaries get less to chew on
#
# What it does not buy: Smart App Control still blocks the app, because SAC
# only honours certificates chaining into Microsoft's Trusted Root Program.
# The PFX stays outside the repo, like the updater key.

$ErrorActionPreference = "Stop"

$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -like "*x64*" } |
  Sort-Object FullName -Descending |
  Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found. Install the Windows SDK signing tools." }

$existing = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert -ErrorAction SilentlyContinue |
  Where-Object { $_.Subject -eq "CN=$Publisher" -and $_.NotAfter -gt (Get-Date) } |
  Sort-Object NotAfter -Descending |
  Select-Object -First 1

if ($existing -and -not $Force) {
  $cert = $existing
  Write-Output "reusing certificate $($cert.Thumbprint)"
} else {
  $cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject "CN=$Publisher" `
    -CertStoreLocation Cert:\CurrentUser\My `
    -KeyUsage DigitalSignature `
    -KeyExportPolicy Exportable `
    -KeyLength 3072 `
    -HashAlgorithm SHA256 `
    -NotAfter (Get-Date).AddYears(3)
  Write-Output "created certificate $($cert.Thumbprint)"
}

# The private key file needs a password even when it never leaves this PC.
if (-not (Test-Path -LiteralPath $PasswordPath)) {
  $bytes = New-Object byte[] 24
  [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
  [IO.File]::WriteAllText($PasswordPath, [Convert]::ToBase64String($bytes))
}
$password = ConvertTo-SecureString ([IO.File]::ReadAllText($PasswordPath).Trim()) -AsPlainText -Force

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $PfxPath) | Out-Null
Export-PfxCertificate -Cert $cert -FilePath $PfxPath -Password $password | Out-Null
Export-Certificate -Cert $cert -FilePath $CerPath -Type CERT | Out-Null
Write-Output "pfx $PfxPath"
Write-Output "cer $CerPath"

# Trust the root so signtool verify passes and UAC can resolve the name.
if (-not (Get-ChildItem Cert:\LocalMachine\Root | Where-Object { $_.Thumbprint -eq $cert.Thumbprint })) {
  Import-Certificate -FilePath $CerPath -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
  Write-Output "root installed for this machine"
} else {
  Write-Output "root already trusted"
}

Write-Output ""
Write-Output "thumbprint: $($cert.Thumbprint)"
Write-Output "sign with: powershell -ExecutionPolicy Bypass -File tools/sign-release.ps1 -Thumbprint $($cert.Thumbprint)"
Write-Output "the .cer is the file a second PC imports into Trusted Root to see the name"

# A second certificate signs App Control policies. It carries the code signing
# EKU plus the Windows System Component Verification EKU that Code Integrity
# policy signing expects. It is separate so the payload certificate stays a
# plain code signing certificate.
$polSubject = "CN=$Publisher policy"
$polDir = Split-Path -Parent $PfxPath
$polPfx = Join-Path $polDir "qc-policy.pfx"
$polCer = Join-Path $polDir "qc-policy.cer"
$polPass = Join-Path $polDir "qc-policy-pass.txt"

$policyCert = Get-ChildItem Cert:\CurrentUser\My -ErrorAction SilentlyContinue |
  Where-Object { $_.Subject -eq $polSubject -and $_.NotAfter -gt (Get-Date) } |
  Sort-Object NotAfter -Descending |
  Select-Object -First 1

if ($policyCert -and -not $Force) {
  Write-Output "reusing policy certificate $($policyCert.Thumbprint)"
} else {
  $policyCert = New-SelfSignedCertificate `
    -Type Custom `
    -Subject $polSubject `
    -CertStoreLocation Cert:\CurrentUser\My `
    -KeyUsage DigitalSignature `
    -KeyExportPolicy Exportable `
    -KeyLength 3072 `
    -HashAlgorithm SHA256 `
    -NotAfter (Get-Date).AddYears(3) `
    -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3,1.3.6.1.4.1.311.10.3.12")
  Write-Output "created policy certificate $($policyCert.Thumbprint)"
}

if (-not (Test-Path -LiteralPath $polPass)) {
  $bytes = New-Object byte[] 24
  [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
  [IO.File]::WriteAllText($polPass, [Convert]::ToBase64String($bytes))
}
$polSecure = ConvertTo-SecureString ([IO.File]::ReadAllText($polPass).Trim()) -AsPlainText -Force

Export-PfxCertificate -Cert $policyCert -FilePath $polPfx -Password $polSecure | Out-Null
Export-Certificate -Cert $policyCert -FilePath $polCer -Type CERT | Out-Null

if (-not (Get-ChildItem Cert:\LocalMachine\Root | Where-Object { $_.Thumbprint -eq $policyCert.Thumbprint })) {
  Import-Certificate -FilePath $polCer -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
  Write-Output "policy root installed, a reboot is required before a policy signed by it is authorized"
} else {
  Write-Output "policy root already trusted"
}
Write-Output "policy thumbprint: $($policyCert.Thumbprint)"
