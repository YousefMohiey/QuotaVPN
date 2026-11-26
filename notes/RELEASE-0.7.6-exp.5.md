# QuotaVPN 0.7.6-exp.5

Experimental. Stable stays at 0.7.5.

## Spaces carry Ethernet

A space can now be layer 2. The app installs a TAP-Windows6 adapter named
`QuotaSpaceLAN`, gives it `10.77.<space>.<n>/24` and a per-member MAC
(`02:51:56:xx:xx:xx`), and switches raw 802.3 frames between members. ARP,
broadcast and multicast are real, so LAN discovery works instead of only
unicast IP.

The switch learns a MAC per peer, sends unicast to that peer only, floods
broadcast, multicast and unknown unicast to everyone else, drops a peer that
claims another peer's MAC, drops duplicated frames that arrive on two paths,
and drops STP, EAPOL and LLDP so link-local chatter stays local. Each peer gets
a 2000 frame per second budget.

Layer 3 is unchanged and still the default. The space owner can switch a space
between modes on the Spaces screen; the mode is stored on the coordinator and
every member picks it up on the next refresh.

## Driver

TAP-Windows6 9.27.0, from the OpenVPN release, attestation-signed by Microsoft
Windows Hardware Compatibility Publisher. 9.27.0 is the floor: 9.26 and earlier
carry CVE-2024-1305, a kernel integer overflow. The installer creates one
adapter and records its GUID in `%APPDATA%\quotavpn\spaces_tap.txt`; uninstall
removes only that device, matched by GUID, never by hardware id.

The driver package is GPLv2. The installer ships the licence text and the
driver source tarball (`tap-windows6-9.27.0.tar.gz`); the MIT app talks to it
through a device handle and links nothing against it.

## Coordinator

The public coordinator is redeployed with `mode`, per-member MACs and
`POST /v1/spaces/{id}/mode`. The database gains a `mode` column on first start.

## Known limits

- Windows to Windows. The phone is still 0.7.5 and has no Spaces screen.
- Multi-peer is full mesh, one Noise session per member. Spaces larger than a
  handful of members will want the relay.
- **Layer 2 does not complete a ping across a real NAT tunnel yet.** In the VPS
  test the punch, the Noise handshake, the TAP adapter and ARP all work: the
  peer answers ARP and the ICMP echo requests reach it, it replies, and the
  switch writes the replies to the adapter. Windows still reports the pings as
  lost. The same switch passes a two-adapter ping on one machine, so the switch
  and the write path are sound and the remaining fault is at the Windows
  receive side or in reply timing. Treat layer 2 as experimental until a second
  real PC proves otherwise. Layer 3 is unaffected.
- The game matrix has not run yet.
- Smart App Control still blocks an unsigned installer. That is unchanged.
