# Radmin inspection, 26 November 2026

Local-only inspection. No VPS or Oracle access. No Radmin adapter was installed.

## Official product page

The official page states **Direct P2P** and "Fast peer-to-peer connections without
server in between", alongside 256-bit AES end-to-end encryption and LAN game
support:

https://www.radmin-vpn.com/

That is the vendor's own confirmation that the data path is peer-to-peer. The
hosted servers are for discovery, membership and the relay fallback, not the
normal path.

## Installer

- URL: https://download.radmin-vpn.com/download/files/Radmin_VPN_2.1.4952.2.exe
- Size: 55,371,080 bytes
- SHA-256: `717A87141FD703BB58BB730D3282E31055BF4EA9C319177ADE9B793974A7D5A4`
- Version: 2.1.4952.2
- Company: Famatech Corp.
- Authenticode: **Valid**, DigiCert Trusted G4 Code Signing RSA4096 SHA384 2021
  CA1, subject `CN=Famatech Corp., O=Famatech Corp., L=Road Town, C=VG`

The installer is in the temp folder, not in the repository. It was not run.

## Why it was not installed

A sing-box tunnel was active on this PC. Radmin installs a virtual Ethernet
adapter, routes `26.0.0.0/8`, and adds broadcast/multicast routes while it runs.
Installing it under a live tunnel can change the routing table and interfere
with the tunnel. The install is deferred until the tunnel is off or a disposable
VM is used.

## Craft Anything triage

Craft Anything 0.6.1 was downloaded from its GitHub release, its SHA-256
verified, and its CLI run against the Radmin installer in Deep mode.

Verdict:

- PE, Windows, x86 (32-bit)
- Entry point `0x4b0028`
- Delphi or C++Builder toolchain hints (an `.itext` section)
- Packed or protected
- Inno Setup installer, detected setup version 6.5.2 (unicode)
- Symbols and names stripped

An `innoextract` build that supports Inno Setup 6.7.0 was tried. It detected
6.5.2 but failed with "Stream error while parsing setup headers", so the payload
was not extracted. This is a tool limitation, not evidence about the product.

Craft Anything is a reverse-engineering and rebuild tool, not a library. A
rebuild would be a separate application, which is not the goal. It was used only
for the installer triage.

## What this adds to the design

- The vendor confirms direct P2P.
- The installer is a packed Delphi/C++Builder application with a 32-bit Inno
  Setup wrapper. The protocol is not exposed by the wrapper.
- The public OpenRad implementation remains the best protocol reference; its
  architecture matches the official direct/relay behaviour.
- The QuotaVPN implementation should continue with the in-house direct path:
  Noise IK session, explicit UDP counters and replay window, Wintun adapter,
  direct-first with relay fallback. No Radmin code is copied.

## Next inspection, when a disposable machine is available

1. Stop the QuotaVPN tunnel.
2. Install Radmin in a VM or on a spare PC.
3. Create a network, join from a second VM, and inspect:
   - adapter name and MAC,
   - the `26.x` address, route and metric,
   - `Get-NetUDPEndpoint` / `netstat -ano` during a direct connection,
   - connection properties: `TCP/out`, `UDP/out` or `TCP/relay`,
   - broadcast and multicast routes.
4. Compare the direct and relay ping with the same two VMs.
