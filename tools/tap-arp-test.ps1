param([string]$Adapter = "QuotaSpaceLAN")

# Which MAC does the TAP receive filter actually use? Write two ARP replies,
# one addressed to the adapter's effective MAC and one to its permanent MAC,
# and see which one Windows installs.

$source = @"
using System;
using System.Runtime.InteropServices;
public static class TapArp {
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sec, uint disp, uint flags, IntPtr tmpl);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool DeviceIoControl(IntPtr h, uint code, byte[] inBuf, uint inSize, byte[] outBuf, uint outSize, out uint ret, IntPtr ov);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool WriteFile(IntPtr h, byte[] buf, uint n, out uint written, IntPtr ov);
  [DllImport("kernel32.dll")]
  public static extern bool CloseHandle(IntPtr h);
  public const uint GENERIC_READ = 0x80000000, GENERIC_WRITE = 0x40000000;
  public const uint FILE_SHARE_READ = 1, FILE_SHARE_WRITE = 2, OPEN_EXISTING = 3;
  public const uint FILE_ATTRIBUTE_SYSTEM = 4;
  public const uint TAP_SET_MEDIA = 0x00220018;
  public static IntPtr Open(string path) {
    IntPtr h = CreateFileW(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, OPEN_EXISTING, FILE_ATTRIBUTE_SYSTEM, IntPtr.Zero);
    if (h == new IntPtr(-1)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    return h;
  }
  public static void Media(IntPtr h, bool up) {
    byte[] b = BitConverter.GetBytes(up ? 1u : 0u); uint r;
    DeviceIoControl(h, TAP_SET_MEDIA, b, 4, null, 0, out r, IntPtr.Zero);
  }
  public static bool Write(IntPtr h, byte[] frame) {
    uint n; return WriteFile(h, frame, (uint)frame.Length, out n, IntPtr.Zero) && n == frame.Length;
  }
}
"@
Add-Type -TypeDefinition $source -Language CSharp

function Mac-Bytes([string]$text) {
  $clean = $text -replace "[:-]", ""
  if ($clean.Length -ne 12) { throw "bad mac $text" }
  $out = @()
  for ($i = 0; $i -lt 12; $i += 2) { $out += [Convert]::ToByte($clean.Substring($i, 2), 16) }
  return [byte[]]$out
}

$nic = Get-NetAdapter -Name $Adapter -ErrorAction Stop
$effective = Mac-Bytes $nic.MacAddress
$permanent = Mac-Bytes $nic.PermanentAddress
Write-Output "effective $($nic.MacAddress) permanent $($nic.PermanentAddress)"

$path = "\\.\Global\$($nic.InterfaceGuid).tap"
$h = [TapArp]::Open($path)
[TapArp]::Media($h, $true)
Start-Sleep -Milliseconds 800

function Arp-Reply([byte[]]$dst, [byte[]]$sha, [string]$spa, [string]$tpa) {
  $spaB = [Net.IPAddress]::Parse($spa).GetAddressBytes()
  $tpaB = [Net.IPAddress]::Parse($tpa).GetAddressBytes()
  return [byte[]]($dst + $sha + 0x08, 0x06, 0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02 + $sha + $spaB + $effective + $tpaB)
}

$shaA = [byte[]](0x02, 0x51, 0x56, 0x00, 0x00, 0x09)
$shaB = [byte[]](0x02, 0x51, 0x56, 0x00, 0x00, 0x0A)
$localIp = ($nic | Get-NetIPAddress -ErrorAction SilentlyContinue | Where-Object { $_.AddressFamily -eq 'IPv4' } | Select-Object -First 1).IPAddress
if (-not $localIp) { $localIp = "10.77.4.2" }

$a = Arp-Reply $effective $shaA "10.77.4.9" $localIp
$b = Arp-Reply $permanent $shaB "10.77.4.10" $localIp
Write-Output "write effective=$([TapArp]::Write($h, $a)) permanent=$([TapArp]::Write($h, $b))"

# Also ask for the adapter's own address over a unicast frame. A request is
# processed even when unsolicited, which tells us whether the receive filter
# accepts unicast to the effective MAC at all.
$spaA = [Net.IPAddress]::Parse("10.77.4.9").GetAddressBytes()
$tpaB = [Net.IPAddress]::Parse($localIp).GetAddressBytes()
$request = [byte[]]($effective + $shaA + 0x08, 0x06, 0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01 + $shaA + $spaA + (0, 0, 0, 0, 0, 0) + $tpaB)
Write-Output "write unicast request=$([TapArp]::Write($h, $request))"
Start-Sleep -Seconds 2

[TapArp]::Media($h, $false)
[TapArp]::CloseHandle($h)

Get-NetNeighbor -InterfaceAlias $Adapter -AddressFamily IPv4 -ErrorAction SilentlyContinue |
  Where-Object { $_.IPAddress -in @("10.77.4.9", "10.77.4.10") } |
  Format-Table IPAddress, LinkLayerAddress, State -AutoSize
