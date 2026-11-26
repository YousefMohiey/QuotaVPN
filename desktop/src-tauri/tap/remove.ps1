# Remove the layer-2 adapter this install created, and only that one.
# Other products install TAP-Windows under the same hardware id, so the device
# is matched by the GUID recorded at install time, never by hardware id.

$ErrorActionPreference = 'SilentlyContinue'
$marker = Join-Path $env:APPDATA 'quotavpn\spaces_tap.txt'
if (-not (Test-Path -LiteralPath $marker)) { exit 0 }

$guid = (Get-Content -LiteralPath $marker -Raw).Trim()
if (-not $guid) { Remove-Item -LiteralPath $marker -Force; exit 0 }

$adapter = Get-CimInstance Win32_NetworkAdapter | Where-Object { $_.GUID -eq $guid } | Select-Object -First 1
if ($adapter -and $adapter.PNPDeviceID) {
  & pnputil /remove-device "$($adapter.PNPDeviceID)" | Out-Null
}
Remove-Item -LiteralPath $marker -Force
exit 0
