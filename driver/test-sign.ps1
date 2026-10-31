# Signs the driver with a self signed certificate for development testing.
# Windows only loads it when test signing is on, which is a boot setting:
#   bcdedit /set testsigning on     then reboot
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$sys  = Join-Path $root "QuotaVpnPkt\QuotaVpnPkt.sys"
$pfx  = Join-Path $root "certs\quotavpn-test.pfx"
$cer  = Join-Path $root "certs\quotavpn-test.cer"
$pass = ConvertTo-SecureString -String "quotavpn-test" -Force -AsPlainText

if (-not (Test-Path $cer)) {
  $cert = New-SelfSignedCertificate -Type CodeSigningCert `
      -Subject "CN=QuotaVPN Test Signing" `
      -KeyUsage DigitalSignature -KeyAlgorithm RSA -KeyLength 2048 `
      -CertStoreLocation "Cert:\CurrentUser\My" -NotAfter (Get-Date).AddYears(3)
  Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $pass | Out-Null
  Export-Certificate -Cert $cert -FilePath $cer | Out-Null
  Write-Host "certificate created"
} else {
  $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq "CN=QuotaVPN Test Signing" } | Select-Object -First 1
}

# The certificate must be trusted, or the loader refuses the signature.
Import-Certificate -FilePath $cer -CertStoreLocation "Cert:\LocalMachine\Root" | Out-Null
Import-Certificate -FilePath $cer -CertStoreLocation "Cert:\LocalMachine\TrustedPublisher" | Out-Null

$signtool = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe"
& $signtool sign /fd SHA256 /f $pfx /p "quotavpn-test" /t http://timestamp.digicert.com $sys
& $signtool verify /pa /v $sys
