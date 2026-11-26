param(
  [string]$Adapter = "QuotaSpaceM0",
  [string]$Peer = "10.77.99.9",
  [int]$ReadSeconds = 5,
  [switch]$MediaOnly,
  [int]$HoldSeconds = 20
)

# M0 probe for the TAP-Windows6 layer-2 adapter.
#
# Opens \\.\Global\{guid}.tap the way the engine will, brings the media up,
# captures the ARP request Windows sends for a neighbour, then injects an ARP
# frame so Windows installs a neighbour entry from it. Reading a frame the OS
# sent and getting the OS to accept a frame we wrote is the whole M0 proof.
#
# Run elevated. Nothing here ships; tap.rs will do the same calls in Rust.

$ErrorActionPreference = "Stop"

$source = @"
using System;
using System.Runtime.InteropServices;

public static class Tap {
  [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  public static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sec, uint disp, uint flags, IntPtr tmpl);
  [DllImport("kernel32.dll", SetLastError = true)]
  public static extern bool DeviceIoControl(IntPtr h, uint code, byte[] inBuf, uint inSize, byte[] outBuf, uint outSize, out uint ret, IntPtr ov);
  [DllImport("kernel32.dll", SetLastError = true)]
  public static extern bool ReadFile(IntPtr h, byte[] buf, uint n, out uint read, ref OVERLAPPED ov);
  [DllImport("kernel32.dll", SetLastError = true)]
  public static extern bool WriteFile(IntPtr h, byte[] buf, uint n, out uint written, ref OVERLAPPED ov);
  [DllImport("kernel32.dll", SetLastError = true)]
  public static extern bool GetOverlappedResult(IntPtr h, ref OVERLAPPED ov, out uint n, bool wait);
  [DllImport("kernel32.dll", SetLastError = true)]
  public static extern IntPtr CreateEventW(IntPtr sec, bool manual, bool init, IntPtr name);
  [DllImport("kernel32.dll")]
  public static extern uint WaitForSingleObject(IntPtr h, uint ms);
  [DllImport("kernel32.dll")]
  public static extern bool CloseHandle(IntPtr h);

  [StructLayout(LayoutKind.Sequential)]
  public struct OVERLAPPED {
    public IntPtr Internal;
    public IntPtr InternalHigh;
    public uint Offset;
    public uint OffsetHigh;
    public IntPtr hEvent;
  }

  public const uint GENERIC_READ = 0x80000000;
  public const uint GENERIC_WRITE = 0x40000000;
  public const uint FILE_SHARE_READ = 1;
  public const uint FILE_SHARE_WRITE = 2;
  public const uint OPEN_EXISTING = 3;
  public const uint FILE_ATTRIBUTE_SYSTEM = 4;
  public const uint FILE_FLAG_OVERLAPPED = 0x40000000;
  public const int ERROR_IO_PENDING = 997;
  // CTL_CODE(FILE_DEVICE_UNKNOWN, function, METHOD_BUFFERED, FILE_ANY_ACCESS)
  public const uint TAP_IOCTL_GET_VERSION = 0x00220008;
  public const uint TAP_IOCTL_GET_MTU = 0x0022000C;
  public const uint TAP_IOCTL_SET_MEDIA_STATUS = 0x00220018;

  public static IntPtr Open(string path) {
    IntPtr h = CreateFileW(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
      IntPtr.Zero, OPEN_EXISTING, FILE_ATTRIBUTE_SYSTEM | FILE_FLAG_OVERLAPPED, IntPtr.Zero);
    if (h == new IntPtr(-1)) {
      throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "CreateFile " + path);
    }
    return h;
  }

  public static uint Ioctl(IntPtr h, uint code, byte[] input) {
    byte[] output = new byte[16];
    uint ret;
    if (!DeviceIoControl(h, code, input, (uint)(input == null ? 0 : input.Length), output, (uint)output.Length, out ret, IntPtr.Zero)) {
      throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "DeviceIoControl " + code.ToString("X8"));
    }
    return BitConverter.ToUInt32(output, 0);
  }

  public static void SetMedia(IntPtr h, bool up) {
    Ioctl(h, TAP_IOCTL_SET_MEDIA_STATUS, BitConverter.GetBytes(up ? 1u : 0u));
  }

  public static int ReadFrame(IntPtr h, byte[] buf, int timeoutMs) {
    OVERLAPPED ov = new OVERLAPPED();
    ov.hEvent = CreateEventW(IntPtr.Zero, true, false, IntPtr.Zero);
    uint n;
    bool ok = ReadFile(h, buf, (uint)buf.Length, out n, ref ov);
    if (!ok) {
      int err = Marshal.GetLastWin32Error();
      if (err != ERROR_IO_PENDING) { CloseHandle(ov.hEvent); return -1; }
      if (WaitForSingleObject(ov.hEvent, (uint)timeoutMs) != 0) { CloseHandle(ov.hEvent); return 0; }
      if (!GetOverlappedResult(h, ref ov, out n, false)) { CloseHandle(ov.hEvent); return -1; }
    }
    CloseHandle(ov.hEvent);
    return (int)n;
  }

  public static bool WriteFrame(IntPtr h, byte[] frame) {
    OVERLAPPED ov = new OVERLAPPED();
    ov.hEvent = CreateEventW(IntPtr.Zero, true, false, IntPtr.Zero);
    uint n;
    bool ok = WriteFile(h, frame, (uint)frame.Length, out n, ref ov);
    if (!ok) {
      int err = Marshal.GetLastWin32Error();
      if (err != ERROR_IO_PENDING) { CloseHandle(ov.hEvent); return false; }
      WaitForSingleObject(ov.hEvent, 2000);
      GetOverlappedResult(h, ref ov, out n, false);
    }
    CloseHandle(ov.hEvent);
    return n == frame.Length;
  }
}
"@

Add-Type -TypeDefinition $source -Language CSharp

$nic = Get-NetAdapter -Name $Adapter -ErrorAction Stop
$guid = $nic.InterfaceGuid
$path = "\\.\Global\$guid.tap"
$ifIndex = $nic.ifIndex
Write-Output "adapter   $Adapter  ifIndex=$ifIndex  mac=$($nic.MacAddress)"
Write-Output "device    $path"

$handle = [Tap]::Open($path)
try {
  $version = [Tap]::Ioctl($handle, [Tap]::TAP_IOCTL_GET_VERSION, $null)
  $mtu = [Tap]::Ioctl($handle, [Tap]::TAP_IOCTL_GET_MTU, $null)
  Write-Output "driver    version=$([math]::Floor($version / 65536)).$($version % 65536)  mtu=$mtu"

  [Tap]::SetMedia($handle, $true)
  Start-Sleep -Seconds 2
  $up = Get-NetAdapter -Name $Adapter
  Write-Output "media     status=$($up.Status)  mac=$($up.MacAddress)"
  Get-NetIPAddress -InterfaceIndex $ifIndex -AddressFamily IPv4 | ForEach-Object { Write-Output "address   $($_.IPAddress)/$($_.PrefixLength) origin=$($_.PrefixOrigin)" }

  if ($MediaOnly) {
    Write-Output "holding   media up for $HoldSeconds seconds"
    Start-Sleep -Seconds $HoldSeconds
    return
  }

  # A neighbour that does not exist, so Windows has to ARP for it.
  $ping = Start-Job -ScriptBlock { param($p) ping.exe -n 2 -w 600 $p } -ArgumentList $Peer
  $buffer = New-Object byte[] 2048
  $frames = @()
  $deadline = (Get-Date).AddSeconds($ReadSeconds)
  while ((Get-Date) -lt $deadline) {
    $n = [Tap]::ReadFrame($handle, $buffer, 500)
    if ($n -gt 0) {
      $frame = $buffer[0..($n - 1)]
      $frames += , $frame
    }
  }
  $null = Wait-Job $ping -Timeout 5
  Remove-Job $ping -Force -ErrorAction SilentlyContinue

  Write-Output "captured  $($frames.Count) frame(s)"
  foreach ($f in ($frames | Select-Object -First 8)) {
    $hex = ($f | ForEach-Object { $_.ToString("X2") }) -join " "
    $etype = if ($f.Length -ge 14) { "0x{0:X2}{1:X2}" -f $f[12], $f[13] } else { "short" }
    Write-Output "  len=$($f.Length) ethertype=$etype $hex"
  }

  # Inject an ARP request claiming the peer address on a peer MAC. The source
  # MAC must differ from the adapter's own, or the stack drops the frame as
  # self-sourced. Windows merges sender information from requests, so this must
  # create a neighbour entry.
  $peerMac = [byte[]](0x02, 0x51, 0x56, 0x00, 0x00, 0x09)
  $spa = [Net.IPAddress]::Parse($Peer).GetAddressBytes()
  $localIp = (Get-NetIPAddress -InterfaceIndex $ifIndex -AddressFamily IPv4 | Where-Object { $_.IPAddress -like "10.77.*" } | Select-Object -First 1).IPAddress
  $tpa = ([Net.IPAddress]::Parse($localIp)).GetAddressBytes()
  $frame = [byte[]](
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    $peerMac[0], $peerMac[1], $peerMac[2], $peerMac[3], $peerMac[4], $peerMac[5],
    0x08, 0x06,
    0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01,
    $peerMac[0], $peerMac[1], $peerMac[2], $peerMac[3], $peerMac[4], $peerMac[5],
    $spa[0], $spa[1], $spa[2], $spa[3],
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    $tpa[0], $tpa[1], $tpa[2], $tpa[3]
  )
  $written = [Tap]::WriteFrame($handle, $frame)
  Write-Output "injected  arp request for $localIp from $Peer on 02-51-56-00-00-09  ok=$written"
  Start-Sleep -Seconds 1

  Get-NetNeighbor -InterfaceIndex $ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Where-Object { $_.IPAddress -eq $Peer } |
    ForEach-Object { Write-Output "neighbour $($_.IPAddress) -> $($_.LinkLayerAddress) state=$($_.State)" }
} finally {
  [Tap]::SetMedia($handle, $false)
  [Tap]::CloseHandle($handle)
  Write-Output "closed    media down"
}
