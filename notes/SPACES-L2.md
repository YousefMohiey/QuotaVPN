# Spaces on layer 2: audit and plan

Goal: a Spaces network that behaves like a real Ethernet LAN, so any LAN game or
LAN-only service works without per-app work. Radmin is the reference: one virtual
adapter, one broadcast domain per network, MAC-addressed peers, no routes for the
games to learn.

This note is the audit of the current layer-3 path and the migration plan. It
ends at the decisions that need a yes or no before code starts.

## What ships today

The Spaces engine is layer 3 end to end.

| Piece | File | What it does |
| --- | --- | --- |
| Address and space record | `src/spaces.rs` | `SpaceRecord` holds `10.77.<space>.<n>/32` and `10.77.<space>.0/24`, the member keys, the coordinator token |
| Engine fragment | `src/spaces.rs` `SpaceTunnel` | Builds the sing-box endpoint that carries space traffic through the main tunnel |
| Transport | `src/spaces_path.rs` | Noise IK, `FRAME_DATA` with a 9 byte header, 16 byte tag, 2 byte length framing on TCP |
| Packet loop | `src/spaces_forward.rs` | One thread, one peer, one adapter. `PacketIo` trait, 15 s keepalive, 2 ms idle sleep |
| Adapter | `desktop/src-tauri/src/wintun.rs` | `WintunIo::new` creates `QuotaSpaceXXXX`, sets `10.77.n.x` with netsh |
| Session owner | `desktop/src-tauri/src/spaces_direct.rs` | Picks one peer, punches UDP/TCP, starts the Noise session, spawns the loop at line 627 |
| Coordinator | `embed/qc-spaces.py` | Membership, addressing, presence |

What that gets right, and should be kept:

- The `PacketIo` trait in `spaces_forward.rs` is already the seam. A different
  adapter is a new implementation of `try_read` and `write`, not a rewrite.
- The Noise session, replay counters, TCP punch and STUN discovery are transport
  work that does not care whether the payload is an IP packet or a frame.
- `wintun.rs` is also the main VPN's adapter. It must not be touched.

What it cannot do, and cannot be patched into:

- No broadcast and no multicast. ARP is answered by the hub for the `/24`, so a
  peer never learns another peer's MAC, and LAN discovery that broadcasts is
  invisible.
- One peer per session, chosen once. A three-PC LAN needs a peer table and a
  destination lookup.
- The address is a `/32` on purpose (`src/spaces.rs` lines 82 to 94): sing-box
  rewrites a `/24` to loopback. A real L2 subnet needs a `/24` on the adapter.
- `spaces_forward::run` treats the payload as opaque, which is fine, but there is
  no source-MAC check, no flooding rule and no per-peer queue.

So the migration is: new adapter, new forwarding logic, same transport and same
coordinator shape with three added fields.

## Driver decision

A virtual Ethernet adapter needs a kernel driver. The options, in order:

1. **TAP-Windows6 (OpenVPN), GPLv2.** NDIS 6.20 miniport, presents 802.3
   Ethernet, exposes `\\.\Global\{guid}.tap` for raw frames, installs with
   `tapinstall.exe install OemVista.inf TAP0901`. Attestation-signed by OpenVPN
   for Windows 10 and 11, so it loads with Secure Boot on, no test signing and no
   EV certificate of ours. This is the only option that works today.

   **Version floor is 9.27.0.** 9.26 and earlier carry CVE-2024-1305, an integer
   overflow in `TapSharedSendPacket` that corrupts kernel memory on a large
   write (CVSS 9.8). OpenVPN fixed it in 9.27.0, which ships inside the OpenVPN
   2.6.10 and later Windows MSI; the standalone `tap-windows-*` installers on
   build.openvpn.net are still 9.24.x and must not be used. Take the package
   from the MSI, verify the `.sys` signature is OpenVPN Technologies with a
   Microsoft attestation countersignature, and gate the build on the file
   version.
2. **Our own NDIS miniport or light-weight filter.** Already attempted and
   parked (`notes/driver-experiment.md`). It cannot load without attestation
   signing, which needs the Hardware Developer Program and an EV certificate.
3. **Wintun.** Layer 3 only. It is why Spaces is where it is.
4. **Npcap.** Capture and injection on an existing adapter, not a new adapter.
   It cannot give a peer a MAC or receive a frame addressed to the virtual LAN.

TAP-Windows6 is therefore the driver. The cost is the licence.

**Licence position to confirm:** TAP-Windows6 is GPLv2. Shipping the signed
driver package inside our installer means shipping GPLv2 binaries, so the release
must carry the licence text and either the driver source or a written offer. Our
app talks to it through a device handle and links nothing against it, so the MIT
code stays MIT. This is the standard reading, and OpenVPN ships it the same way,
but it is a product decision, not a code decision. If it is refused, there is no
permissive L2 driver to fall back on, and the migration stops at M0.

Install shape (all confirmed in M0):

- Ship `amd64/OemVista.inf`, `tap0901.sys`, `tap0901.cat` and `devcon.exe`
  from the 9.27.0 `dist.win10.zip`. That release has no `tapinstall.exe`; it
  ships `devcon.exe`, which does the same job.
- NSIS hook, elevated (the manifest already asks for it):
  `devcon.exe install OemVista.inf TAP0901`. No reboot, no trust prompt, because
  the package is signed by Microsoft Windows Hardware Compatibility Publisher.
  One adapter per install, named `QuotaSpaceLAN`; the engine renames it if the
  driver install left the default name.
- Never reuse an adapter that OpenVPN or another product installed. Install our
  own instance and record its GUID under `%APPDATA%\quotavpn\spaces_tap.txt`.
- Uninstall: `devcon.exe remove TAP0901`, then `pnputil /delete-driver
  oem<NN>.inf /uninstall /force`, where `oem<NN>.inf` is found by provider
  (`TAP-Windows Provider V9`), never hardcoded.
- TAP supports several instances, so one space at a time and OpenVPN side by
  side both work.

## Target architecture

```
 game / browser / SMB
        |
  QuotaSpace<id>  (TAP, Ethernet, 10.77.<space>.n/24, no gateway)
        |  raw 802.3 frames
  spaces_switch  (per space)
    MAC table: MAC -> peer
    unicast: one peer
    broadcast / multicast / unknown unicast: every other peer
    ingress check: source MAC must not be owned by another peer
        |
  peer table: peer id -> Noise session (UDP punched, TCP punched, relay later)
        |
  spaces_path (unchanged framing and crypto)
```

The switch lives in `src/spaces_switch.rs`, next to `spaces_forward.rs`. The old
loop stays for layer-3 spaces during the migration and is deleted at M6 if no
space still needs it.

## Switch rules

1. **Learning.** On ingress from peer P, map `source MAC -> P` with a 5 minute
   idle expiry. A frame whose source MAC is currently mapped to another peer is
   dropped and logged; that is the loop and spoof guard.
2. **Unicast.** Destination MAC known and not P: send to that peer only.
3. **Flood.** Broadcast (`ff:ff:ff:ff:ff:ff`), multicast (low bit of the first
   octet set) and unknown unicast: send to every member except P.
4. **No reflection.** A frame never goes back to the peer it came from.
5. **Duplicate suppression.** Key on `(source MAC, sequence-ish hash of the
   frame)` in a 1 second cache. Needed once relay and direct paths can both
   deliver the same frame.
6. **Rate limit.** Per peer, 2000 frames per second and a 1 MB burst, then drop.
   A chatty game must not stall the loop.
7. **Isolation.** One switch instance per space. A frame from space A can never
   reach space B, even if the coordinator returns a wrong member list.
8. **Link-only frames stay local.** Drop 802.1D STP and 802.1X EAPOL (destination
   `01:80:C2:00:00:00` to `01:80:C2:00:00:0F`) and LLDP (`ethertype 0x88CC`) at
   ingress. Windows sends LLDP and IPv6 multicast the moment the adapter comes
   up; forwarding LLDP to every peer is noise, and spanning tree frames have no
   meaning on a hub-and-spoke mesh.

Addressing and MACs:

- Keep the coordinator's IPv4 plan: `10.77.<space>.<n>/24`, no default gateway,
  so the adapter can never steal internet traffic.
- One MAC per member, published by the coordinator. A stable, per-member MAC is
  confirmed feasible: the INF exposes `NetworkAddress` as an advanced property,
  and `Set-NetAdapterAdvancedProperty -RegistryKeyword NetworkAddress
  -RegistryValue 025156000001` produced `02-51-56-00-00-01` on the adapter. Use
  the `02:51:56:` prefix and the member index, so the OUI is locally
  administered and collisions are impossible.
- No DHCP server. Every member has a static address, as Radmin does. Set it with
  `netsh interface ipv4 set address ... gateway=none`; the PowerShell cmdlet
  fails on a DHCP-enabled adapter (see M0).
- Keep IPv6 enabled. Link-local works over real Ethernet with no router
  advertisements, and IPv6 LAN discovery then works too.

MTU: set the TAP adapter to 1380. The frame is then 1394 bytes, the Noise header
and tag add 26, and the datagram stays under a 1500 byte path. 1500 on the
adapter would fragment on any path with a normal MTU.

## Changes by file

| File | Change |
| --- | --- |
| `desktop/src-tauri/src/tap.rs` | New. Open the device, read and write frames, wait on the overlapped event, set MTU |
| `desktop/src-tauri/src/spaces_direct.rs` | Adapter factory by space mode; peer table instead of one peer; spawn the switch |
| `desktop/src-tauri/src/spaces.rs` | Pass `mode`, `mac`, `mtu` into the engine; status carries them to the UI |
| `src/spaces.rs` | `SpaceMember` gains `mac`; `SpaceRecord` gains `mode`; `SpaceTunnel` stays for l3 |
| `src/spaces_switch.rs` | New. Learning, flooding, duplicate cache, rate limit, tests with in-memory peers |
| `src/spaces_forward.rs` | Kept for l3, or deleted at M6 |
| `embed/qc-spaces.py` | Store and return `mode`, `mac`, `mtu`; reject an l2 join from a client that did not send them |
| `desktop/ui-next/src/pages/Spaces.tsx` | Show mode, adapter name and MAC; the connect copy stops saying "direct" when it is a LAN |
| `desktop/src-tauri/windows/nsis-hooks.nsh` | Driver install and remove |
| `tools/build-installer.sh` | Bundle the driver package |
| `notes/STATUS.md`, `AGENTS.md` | Record the driver, the licence and the new path |

Compatibility: the phone is 0.7.5 and knows nothing about Spaces, so the
coordinator must keep the old record shape working. An l2 space is only joinable
by a client that sent `mode=l2` and a MAC. A Windows client that sees `mode=l3`
keeps today's behaviour.

## M0 result

Done on this PC on 26 November 2026. The driver is installed and left in place
for M1.

- Package: `dist.win10.zip` from the tap-windows6 9.27.0 release. `tap0901.sys`
  is 02/27/2024, file version 9.27.0.0, signature `Valid`, signer
  `Microsoft Windows Hardware Compatibility Publisher`, on both the `.sys` and
  the `.cat`.
- Install: `devcon.exe install OemVista.inf TAP0901`. Device node created, no
  prompt, no reboot. The adapter appears as `TAP-Windows Adapter V9` with a
  generated `00-FF-...` MAC and status `Disconnected`.
- Device path: `\\.\Global\{5C185A64-6158-40A7-BEF2-F20838802D67}.tap`, taken
  from `Get-NetAdapter.InterfaceGuid`.
- Media: the adapter stays `Disconnected` and installs no routes until
  `TAP_IOCTL_SET_MEDIA_STATUS` is set to 1. After that it is `Up` and frames
  flow. Setting it back to 0 returns it to `Disconnected`.
- MTU: the INF exposes `MTU` as an advanced property with a 1500 maximum. Set to
  1380 and `TAP_IOCTL_GET_MTU` reported 1380.
- MAC: the INF exposes `NetworkAddress`. Setting `025156000001` produced
  `02-51-56-00-00-01` on the adapter. A stable per-member MAC is real.
- Address: `netsh interface ipv4 set address name="QuotaSpaceM0" source=static
  address=10.77.99.2 mask=255.255.255.0 gateway=none` worked on the DHCP-enabled
  adapter. `New-NetIPAddress` failed with "Inconsistent parameters PolicyStore
  PersistentStore and Dhcp Enabled", which is why `wintun.rs` already uses netsh
  with a retry loop.
- Profile: a new TAP is classified `Public`. With media up,
  `Set-NetConnectionProfile -InterfaceIndex 54 -NetworkCategory Private` changed
  it to `Private`. It has to be reapplied on every start.
- Read: the probe captured 82 frames in five seconds. LLDP (`0x88CC`), IPv6 MLD
  and neighbour discovery from Windows itself, and the ARP duplicate address
  probes for the adapter address.
- Write: an injected ARP request from `10.77.99.9` on `02-51-56-00-00-09` made
  Windows install `10.77.99.9 -> 02-51-56-00-00-09 state=Stale`. Reading and
  writing the Windows stack are both proven.
- Trap: a frame whose source MAC equals the adapter's own MAC is dropped by the
  stack. The switch must never reflect a frame to its sender and must never
  rewrite the source MAC.
- Uninstall: `devcon.exe remove TAP0901` removed the node cleanly. The driver
  package stayed in the store as `oem39.inf`; a full uninstall also needs
  `pnputil /delete-driver`, matched by provider because the `oem` number differs
  per machine.

The probe is `tools/tap-probe.ps1`. It is a diagnostic, not shipped code.

## Progress (26 November 2026)

M0 is done. The driver package, the install and remove commands, and the traps
are recorded above.

The first code slice is in the tree:

- `desktop/src-tauri/src/tap.rs` opens the recorded adapter, sets the MAC, MTU
  and address, brings the media up, sets the profile to Private, and implements
  `PacketIo`. It leaves other products' TAP adapters alone: the GUID is recorded
  in `spaces_tap.txt`, and a fallback only adopts the sole TAP adapter.
- `src/spaces_switch.rs` is the learning switch, with tests for unicast,
  flooding, spoofing, duplicates, control frames and the rate limit.
- `src/spaces.rs` carries `mode` (`l3` or `l2`) and derives each member's MAC
  from its id as `02:51:56:xx:xx:xx`.
- `desktop/src-tauri/src/spaces_direct.rs` picks the TAP adapter and the switch
  when the space is `l2`, and keeps Wintun plus the old loop for `l3`.
- `embed/qc-spaces.py` stores and returns `mode`, returns each member's MAC, and
  has `POST /v1/spaces/{id}/mode` for the owner. The desktop command is
  `space_set_mode`.

The ignored test `tap::tests::the_adapter_comes_up_and_carries_a_frame` was run
on this PC against a real adapter and passed, so the Rust device path is proven,
not just the PowerShell probe.

Still open before this is a usable release:

1. Multi-peer sessions. The switch already takes a peer list; `start` still
   connects one peer. The next step is to reuse the rendezvous and punch for
   every online member and hand the vector to the switch.
2. Installer: ship the driver, create the adapter with `devcon`, name it
   `QuotaSpaceLAN`, record the GUID, and remove it on uninstall.
3. UI: show the mode, the adapter and the MAC on the Spaces page, and let the
   owner switch a space to `l2`.
4. The application matrix from M4.

## Milestones

Each one ends with something observable, not with code that compiles.

**M0, driver probe (half a day). Done.** Extracted 9.27.0 from `dist.win10.zip`,
verified the signatures, installed, opened the device, set media, read 82 frames,
injected an ARP frame that Windows accepted, set the profile to Private, and
removed the device cleanly. Evidence and the traps are in the M0 result above.
The go decision is taken; the remaining milestones can start.

**M1, one peer over Ethernet (2 days).** `tap.rs` behind the existing
`PacketIo` trait. The engine builds a TAP adapter when the space says `l2` and a
Wintun adapter when it says `l3`. Two PCs, static addresses, `ping` works and
`arp -a` shows the peer's MAC. No switch yet, one peer.

**M2, the switch (3 days).** Peer table, MAC learning, unicast, flood,
ingress check, duplicate cache, rate limit. Unit tests drive frames between
in-memory peers. Then three PCs: all-to-all ping, a broadcast reaches both, a
frame never returns to its sender.

**M3, presence and ownership (2 days).** Coordinator carries `mode`, `mac`,
`mtu`. The Spaces supervisor keeps the engine alive without the screen. UI shows
the adapter, MAC, mode and peer count. Restart the app and the LAN is back
without opening the page.

**M4, applications (2 days).** A fixed matrix, at least five independent titles,
each hosted by a different PC in turn. Record discovery, hosting and joining per
title. Fix only what the matrix exposes; no per-game code.

**M5, resilience (2 days).** Duplicate suppression across two paths, UDP to TCP
failover on the existing punch, adapter and firewall self-repair, network
profile repair, MTU clamp when a path is smaller.

**M6, packaging (1 day).** Driver in the installer, uninstall removes it, GPL
notices and source offer, release notes, and a gate that fails the build when the
driver payload is missing.

Rough total: 12 working days, one experimental release at the end, stable 0.7.5
untouched.

## Test matrix for M4

Pick titles that use different discovery mechanisms, not five games with the same
engine:

| Title | Discovery | Why it is on the list |
| --- | --- | --- |
| Warcraft III | UDP broadcast | The classic L2 case |
| Age of Empires II DE | UDP broadcast plus LAN lobby | Large broadcast volume |
| Terraria | UDP broadcast on 7777 | Simple, fast to retest |
| Minecraft Java | Multicast 224.0.2.60 | The multicast case |
| OpenTTD | UDP broadcast | Small frames, many of them |
| Stardew Valley | Steam or IP join | Tests the non-broadcast path |
| SMB share and RDP by name | NetBIOS and mDNS | Proves name resolution over L2 |

If multicast 224.0.2.60 works, the flooding rule is right. If ARP resolves and
name resolution works, the adapter profile and firewall handling are right.

## Risks

1. **GPLv2.** Blocks M0 until accepted. No permissive alternative exists.
2. **Driver trust.** We depend on OpenVPN keeping the package attestation-signed.
   The public standalone installer is stale and vulnerable, so the package comes
   from the OpenVPN MSI. If a Windows update refuses it, there is no fallback in
   this plan.
3. **Network profile.** A TAP adapter that Windows classifies as Public will
   drop the inbound traffic the games need. The engine must set Private and add
   the firewall rule, and must redo it after a driver reinstall.
4. **Two adapters at once.** Wintun carries the main tunnel, TAP carries the
   space. No gateway on TAP plus a high interface metric keeps the default route
   where it is.
5. **Broadcast storms.** A misbehaving game can flood every peer. The per-peer
   rate limit is not optional.
6. **Antivirus.** A new NDIS adapter installed by an unsigned installer is the
   kind of step heuristics watch. This is the same SmartScreen problem as today,
   not a new one, but the driver step raises the cost of a blocked install.
7. **OpenVPN coexistence.** Do not adopt or rename an existing TAP instance.
   Install our own and keep its GUID.
8. **Phone.** The phone cannot carry L2 without a merged VpnService, so l2
   spaces are Windows to Windows until that work is scheduled.

## Decisions needed

1. Accept GPLv2 TAP-Windows6 in the installer, with licence text and a source
   offer. Without this, M0 does not start.
2. Confirm the scope: Windows to Windows l2 spaces first, phone stays l3.
3. Confirm the release shape: one experimental build after M2 to test the frame
   path, then the full experimental release after M6.

## What this plan refuses to do

- No per-game patches. If a title needs a special rule, the switch is wrong.
- No routing tricks over the layer-3 path. Half an L2 network is worse than none.
- No changes to `wintun.rs` or the main tunnel.
- No test signing, no Secure Boot changes, no self-signed driver.
