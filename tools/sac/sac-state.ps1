# Report the Smart App Control mode and which CI policy is actually enforced.
# Read-only: safe to run without elevation.

$key = "HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy"
$value = (Get-ItemProperty -Path $key -ErrorAction SilentlyContinue).VerifiedAndReputablePolicyState

$names = @{ 0 = "Off"; 1 = "Enforce"; 2 = "Evaluation" }
if ($null -eq $value) {
  Write-Output "VerifiedAndReputablePolicyState is not present. SAC was never provisioned on this PC."
} else {
  Write-Output ("VerifiedAndReputablePolicyState = {0} ({1})" -f $value, $names[[int]$value])
}

$enforced = & citool.exe -lp 2>$null | Select-String -Pattern "Friendly Name: VerifiedAndReputable|Is Currently Enforced: true" |
  ForEach-Object { $_.Line.Trim() }
if ($enforced) {
  Write-Output "Active policies:"
  $enforced | ForEach-Object { Write-Output ("  " + $_) }
}

if ($value -ne 0) {
  Write-Output "Unsigned installers are blocked while this is 1, and usually while it is 2."
} else {
  Write-Output "Unsigned installers run."
}
