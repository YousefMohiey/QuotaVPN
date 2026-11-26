# OpenRad integration assessment

Date: 25 November 2026. OpenRad 2.4.5, MIT.

> **Decision, 25 November 2026:** QuotaVPN will build the direct-first data
> plane in-house. OpenRad is not shipped as a sidecar, a second app or a driver
> bundle. This document is kept for the integration blockers and as a protocol
> reference. The Windows sidecar sections below are the rejected option, not the
> current plan.

This is the verdict after reading the OpenRad source, its Windows release, its
CLI and its server. It answers one question: can QuotaVPN Spaces be built by
copying OpenRad?

Short answer: **do not copy the repository into QuotaVPN.** OpenRad is a full
second VPN product: a TAP driver, an elevated per-user service, its own identity
and server protocol, a desktop app, an Android app and a Go coordination server.
The reusable part is the engine and the protocol shape, and the fastest way to
prove it is a Windows sidecar spike, not a source merge.

## What OpenRad actually is

- Rust `openrad-client` library, CLI and eframe desktop app.
- Windows TAP-Windows6 virtual Ethernet adapter, component `tap0901`.
- Per-user background service with a local named pipe or Unix socket.
- Identity registration against a Radmin-compatible coordination server.
- Direct TCP, reliable UDP and relay peer transports.
- A self-hosted Go coordination server with SQLite, RSA server key, control
  port 17301, relay port 17302 and UDP endpoint discovery on 17301 to 17399.
- Android client in pure Kotlin with its own `VpnService`.
- Paper plugin and Fabric mod for Minecraft.
- MIT license, but beta. Its own docs say protocol compatibility is not a
  completed security audit.

The wire protocol is Radmin's control protocol, not a generic VPN protocol. The
client registers an identity, gets a `26.x.y.z` address, joins named networks
and opens peer channels through the coordinator.

## Why it is not a drop-in copy

### 1. It is a whole app, not a crate with one connect function

The engine entry point is `runtime::run(identity, modulus, options, commands,
stop, report)`. It expects a registered identity, the server's RSA modulus, a
command channel and a report callback. Around it sit `daemon`, `session`,
`peer`, `tap`, `identity`, `keyring`, `network`, `protocol` and a local control
socket. There is no small "join this space and give me a TAP fd" API designed
for a host app.

You can embed it, but you are embedding OpenRad's lifecycle, not calling a
library function.

### 2. Windows needs TAP-Windows6 and elevation

OpenRad requires the official TAP-Windows6 driver, a dedicated adapter named
`OpenRad`, and an elevated process for the TAP setup. QuotaVPN ships Wintun and
an elevated helper. Adding a second kernel driver, a second adapter and a
second elevated process is possible, but it is an installer, signing and
antivirus project. OpenRad's Windows support is experimental and was tested on
Windows 10; this machine is Windows 11.

### 3. Android cannot run two VpnServices

Android allows one active VPN. QuotaVPN owns it. OpenRad's Android client owns
its own. A merged product requires porting OpenRad's pure-Kotlin engine into
QuotaVPN's `VpnService`, or accepting a second app. OpenRad's Android README
says its peer channels are direct TCP or relay; it does not list direct UDP.
That is a separate integration effort from Windows.

### 4. It has its own identity, server and network model

OpenRad has registered identities, Radmin-compatible networks, passwords and
roles. QuotaVPN has cards, invites, spaces and a coordinator. Copying OpenRad
means either replacing the Spaces control plane or wrapping OpenRad networks
behind the Spaces UI. The second is possible, but it is an adapter project.

### 5. It conflicts with QuotaVPN's no-scheduled-tasks rule

OpenRad's Windows client registers a logon scheduled task for auto-start.
QuotaVPN has a hard rule against scheduled tasks after Defender quarantined the
app twice. Any integration must disable or remove that task and start the
service on demand. That is a small but real packaging difference.

### 6. It is beta and unaudited

OpenRad is an impressive project, but it is not a shipping VPN vendor. The
source is public, the tests are extensive, and it interoperates with Radmin, but
its own documentation does not claim a security audit. Putting it on the data
path of paying customers without a review is a risk.

## The three real options

| Option | What it means | Effort | Risk |
| --- | --- | --- | --- |
| Windows sidecar | Install OpenRad 2.4.5, point it at a self-hosted OpenRad server, control it from QuotaVPN with `openrad.exe` CLI and JSON. Keep the Spaces UI as a wrapper. | Days for a spike, weeks for a shippable adapter | TAP driver, second service, scheduled task, beta Windows support |
| Embed the library | Add `openrad-client` to the Tauri backend, supply identity and modulus, run its runtime in a thread, attach a TAP adapter. | Weeks to months | TAP integration, lifecycle, protocol, UI mapping |
| Port the protocol | Keep QuotaVPN's coordinator and UI, implement direct UDP hole punching, relay fallback and an Ethernet TAP in our own stack, using OpenRad and MiniLAN as references. | Months | Most control, most work, security review needed |
| Copy the whole app | Replace Spaces with OpenRad's desktop and Android apps. | Not viable as one product | Two apps, two identities, two UIs |

## The sidecar spike, exactly

This proves the data path before any source integration.

**First spike: use OpenRad's default server.** Do not deploy the VPS server
yet. The default server removes the arm64 build, Go 1.26.4, TLS, ports and
server security from the experiment and isolates the one question that matters:
does a direct peer path work between the two PCs? Self-host only after direct is
proven.

1. Install OpenRad 2.4.5 on both PCs and use the default server. Deploy the
   self-hosted server later, before product integration:
   - Build `openrad-server` from the repo. The published Linux release is
     x86_64 only, and the QuotaVPN VPS is an Oracle A1 (arm64), so build with
     `GOOS=linux GOARCH=arm64`. The server's `go.mod` requires Go 1.26.4, which
     is not installed on this PC; build on the VPS or install that Go version
     there.
   - Ports: TCP 17301 control, TCP 17302 relay, UDP 17301 to 17399 endpoint
     discovery.
   - `--public-ip` must be the VPS public IPv4. Endpoint discovery wants two
     distinct routable IPs; with one VPS IP, expect weaker NAT classification
     and more relay fallback. A second IP or a second small VPS is the clean
     answer.
   - Protect the SQLite database: it holds the server RSA private key.
2. Install OpenRad 2.4.5 on two Windows PCs. The setup installs TAP-Windows6
   and the per-user service.
3. Point both clients at the server:
   ```
   openrad.exe server set --host <vps-ip> --port 17301 --modulus modulus.bin
   openrad.exe init --node-name <device>
   openrad.exe start
   ```
4. Create and join a private network:
   ```
   openrad.exe create "spaces-test" --password-file pass.txt
   openrad.exe join "spaces-test" --password-file pass.txt
   ```
5. Measure:
   - `openrad.exe --json peers` shows the transport, direct TCP, direct UDP or
     relay.
   - `openrad.exe ping <RID>` gives the RTT.
   - Start a Minecraft server on one PC and check the LAN list and the in-game
     ping on the other.
6. If the direct path works and the ping is close to Radmin, integrate. If it
   falls back to relay, the problem is the network, and copying code will not
   fix it.

## If the spike works: the adapter design

Keep the QuotaVPN Spaces screen and its invite model. Do not ship OpenRad's UI.

- A Space becomes an OpenRad private network:
  - Generated network name: `qv-<space id>`.
  - Generated password stored with the space record.
  - Invite code carries the network name and password (new prefix, for example
    `qvo1:`), so the friend can join without typing a separate password.
- `space_create` calls `openrad create`, then stores the name and password.
- `space_join` calls `openrad join`.
- `space_refresh` calls `openrad --json status` and `openrad --json peers` and
  maps RID, VIP, name, online state and transport into the existing member
  rows. The address becomes `26.x.y.z`, not `10.77.x.y`.
- Connect starts the OpenRad service. Disconnect stops it. The main QuotaVPN
  tunnel is independent; the space can work without it, which is closer to
  Radmin.
- Leave and delete call `openrad leave` and `openrad delete`.
- The UI shows the transport (direct TCP, direct UDP, relay) next to each peer.
  That is the honest answer to "will it be 9 ms".
- Broadcast and multicast work through TAP, so Minecraft "Open to LAN" should
  appear in the LAN list.

The current hub and coordinator can stay as a fallback for the phone and for
users who cannot install TAP, but they are no longer the primary Windows path.

## Android plan

Do not promise a phone release until the Windows spike works. The options are:

1. Port OpenRad's pure-Kotlin engine into QuotaVPN's `VpnService`. It already
   has a pure-Kotlin engine, an L3 bridge and broadcast support. This is the
   only way to keep one app and one VPN session.
2. Ship the OpenRad Android app separately for Spaces. This is a second app and
   a second identity, not a QuotaVPN feature.
3. Keep the current relay-only Space on the phone until the Kotlin port is
   ready.

## Licensing and legal notes

- OpenRad is MIT. Reuse requires keeping its copyright and license notice, and
  the notices for its dependencies. Its README credits `asimplestray` for the
  cryptography.
- `baptisterajaut/radmin-vpn-linux` is GPL-3.0. Learn from its L2 and adapter
  documentation, do not copy its code into a non-GPL product.
- MiniLAN is MIT. Its `PROTOCOL.md` is a useful smaller reference, but its
  implementation is Node and Kotlin, not Rust.
- OpenRad implements the Radmin control protocol for interoperability. A
  commercial product should have that reviewed before shipping.

## Verdict

Copying the OpenRad repository into QuotaVPN is the wrong first move. It would
add a second VPN app, a second driver, a second identity and a second Android
VpnService, and it would still need the Spaces UI mapped onto OpenRad networks.

The right first move is the Windows sidecar spike against a self-hosted OpenRad
server. It answers the only question that matters, direct versus relay and
Minecraft LAN discovery, with days of work instead of months. If it passes,
build the adapter and keep the Spaces UI. If it fails, the network is the
problem, and a source copy would have failed the same way.
