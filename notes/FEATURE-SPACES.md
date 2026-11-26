# QuotaVPN Spaces

A private virtual LAN for QuotaVPN users: create a network, invite people, and
reach each other by a stable address the way Radmin VPN does, without exposing
anything to the public internet.

Status: design and first implementation slice, 25 November 2026. Desktop only
in this slice. The phone keeps its 0.7.5 UI until the data plane and the screen
are ready.

> **Research update, 25 November 2026:** the relay-only data plane in this
> document is a working first slice and the fallback path, not the final
> Radmin-equivalent design. Radmin is direct-first: peers punch through NAT and
> talk directly, with a TCP relay only when that fails. It also carries
> Ethernet, so Minecraft LAN discovery works. The current Spaces slice is layer
> 3 and always relays through the VPS, so it cannot match a 9 ms direct link.
> The evidence, the GitHub implementations and the revised direction are in
> section 12. Read that before extending the data plane.

## 1. The problem

Two people with QuotaVPN installed cannot reach each other. Every card is a
client of one server, and the server treats clients as separate customers. There
is no shared address space, no roster, and no way to open a file share, a game
lobby, RDP, or an HTTP server for a friend.

Radmin VPN solves this by putting every member of a network on one virtual
adapter, assigning each a private address, and carrying traffic between them
through Famatech's infrastructure. The product is free, Windows only, and it is
a separate product from Radmin Remote Control. Its own description is a virtual
LAN: devices anywhere appear to share a local network, traffic is encrypted with
256-bit AES, and LAN games work because broadcast and multicast are carried
too. It also keeps a no-logging policy and a built-in firewall for gaming
networks.

Sources:

- https://www.security.org/vpn/radminvpn
- https://www.radmin-vpn.com/security/
- https://github.com/baptisterajaut/radmin-vpn-linux

The Linux launcher for the Windows client is useful evidence for the shape of
the product: it routes `26.0.0.0/8`, sends broadcast and multicast through the
VPN adapter so LAN games can find each other, and has an option to turn those
broadcast routes off. That is layer-2 behaviour on top of a relay.

QuotaVPN does not need to copy Radmin's implementation. It needs to copy the
useful outcome: a small group of people, a private address each, and direct
reachability by IP, with the same reliability from Egypt.

## 2. What the other products teach

Tailscale separates the control plane from the data plane. A coordination
service authenticates devices, assigns addresses, distributes keys and policies;
the devices then speak WireGuard directly. When NAT traversal fails, DERP relays
carry the encrypted packets. It is layer 3, so broadcast and multicast do not
work.

- https://tailscale.com/docs/concepts/control-data-planes
- https://tailscale.com/kb/1232/derp-servers

NetBird has the same split with more moving parts: a management service, a
signal service built on Pion ICE, a TURN relay, and WireGuard as the data plane.
It is self-hostable and open source, but a deployment is four services.

- https://docs.netbird.io/about-netbird/how-it-works

ZeroTier is the outlier. It emulates Ethernet, so broadcast, multicast and
non-IP protocols work, which is why it is the choice for LAN game discovery and
old industrial equipment. The cost is a custom protocol and a virtual L2 stack
on every platform. Layer 2 is the expensive part, not the encryption.

- https://tailscaleguide.com/posts/tailscale-vs-zerotier-vs-netbird-mesh-vpn-compared

RustDesk is not a LAN product, but its connection flow is the reference for
remote desktop later: an ID server for rendezvous, hole punching first, a relay
when punching fails. That is the same control/relay pattern, applied to a
screen stream instead of an IP packet.

- https://rustdesk.com/docs/en/self-host/

Lessons for QuotaVPN:

1. A coordination service is unavoidable. Identity, addressing, invites and
   revocation need one authoritative place.
2. A relay is the reliability floor. Direct peer-to-peer is an optimisation, not
   a prerequisite.
3. Layer 3 covers the useful cases now: file shares, RDP, SMB, HTTP, SSH, game
   servers joined by IP. Broadcast is a separate project.
4. The relay must not become a public proxy. It should be reachable only by
   members, and only for the space address range.

## 3. The chosen design

Spaces is a layer-3 virtual LAN carried by WireGuard, coordinated by a small
service on the existing QuotaVPN VPS, and reached through the main QuotaVPN
tunnel so the handshake never meets the ISP's DPI.

```
        Windows PC A                         Windows PC B
   +--------------------+              +--------------------+
   | QuotaVPN UI        |              | QuotaVPN UI        |
   |  Spaces screen     |              |  Spaces screen     |
   +---------+----------+              +---------+----------+
             |                                   |
   +---------v----------+              +---------v----------+
   | amnezia-box engine |              | amnezia-box engine |
   |  main TUN          |              |  main TUN          |
   |  VLESS / Hy2 / AWG |              |  VLESS / Hy2 / AWG |
   |  space endpoint    |              |  space endpoint    |
   +---------+----------+              +---------+----------+
             |  WireGuard inside the main tunnel
             |  (detour: proxy)
             v                                   v
   +------------------------------------------------------+
   |                QuotaVPN VPS, TCP 443                  |
   |  Xray inbound -> freedom outbound -> 10.77.0.1:51821 |
   |                                                      |
   |  qcspaces (kernel WireGuard, 10.77.0.1/16)            |
   |    peer A 10.77.3.2  <-->  peer B 10.77.3.3           |
   |    per-space iptables isolation                       |
   |                                                      |
   |  qc-spaces coordinator, 127.0.0.1:8445                |
   |    SQLite: spaces, members, tokens, invite hashes     |
   +------------------------------------------------------+
```

### 3.1 Why the hub is reached through the main tunnel

Egypt blocks plain WireGuard handshakes. That is why the desktop's WireGuard
transport is AmneziaWG on UDP 53 in the first place. A second plain WireGuard
tunnel on a public UDP port would fail from the same networks.

The space endpoint therefore sets `detour: "proxy"`. Its WireGuard UDP packets
are dialled through the main QuotaVPN outbound, which is already VLESS, Hysteria2
or AmneziaWG. The hub endpoint is `10.77.0.1:51821`, the hub interface's own
address, not loopback. A proxy transport sends the packet to that address as a
local destination on the VPS; the main WireGuard transport reaches it through
`wg0`, where the kernel also treats it as local. Loopback would only work for
the proxy transports and would silently break Spaces on the WireGuard card.
There is no new public UDP port, no second obfuscation layer, and no DPI
signature to match.

The trade-off is explicit: **Spaces only works while the main QuotaVPN session is
up**. The traffic spends the card's quota, like any other tunneled traffic. That
is acceptable for the first release because QuotaVPN is the product the user
already runs. A direct path can be added later without changing the UI.

The hub listens on all interfaces at the kernel level because WireGuard has no
bind-address option. An iptables rule accepts UDP 51821 only from loopback and
the main WireGuard subnet (`10.8.0.0/24`), the two paths the main tunnel can
take, and drops everything else. The Oracle security list never opens the port.

### 3.2 Address plan

- One `/16` for all spaces: `10.77.0.0/16`.
- One `/24` per space: `10.77.<n>.0/24`, `n` from 1 to 254.
- The hub is `10.77.<n>.1`.
- Members get `10.77.<n>.2` through `10.77.<n>.254`, which is 253 members per
  space.
- The hub interface carries `10.77.0.1/16`, which gives the kernel one route for
  the whole range. Each peer's `AllowedIPs` is its own `/32`.
- Client routes only its space's `/24`, never a default route. The main tunnel's
  routing rules send that `/24` to the space endpoint; everything else keeps its
  existing path.

The `/24` per space is what keeps two unrelated networks apart. The coordinator
adds a firewall rule per space:

```
-A FORWARD -i qcspaces -o qcspaces -s 10.77.<n>.0/24 -d 10.77.<n>.0/24 -j ACCEPT
-A FORWARD -i qcspaces -o qcspaces -j DROP
```

The DROP rule is appended once and the ACCEPT rules are inserted above it. A
member of space 3 cannot reach space 4 even though both share one interface.

`10.77.0.0/16` is uncommon enough to avoid most home routers. It is not
guaranteed: a user whose LAN already uses `10.77.3.0/24` will conflict with
space 3. The join response includes the subnet so the UI can show it. Refusing a
conflicting subnet before activation, and letting an owner choose one at
creation, are phase-2 items; the first slice shows the subnet and leaves the
decision to the person who can see their own network.

### 3.3 Client configuration

The engine already writes one sing-box document. A space adds one top-level
endpoint and one route rule:

```json
{
  "endpoints": [
    {
      "type": "wireguard",
      "tag": "space",
      "address": ["10.77.3.7/32"],
      "private_key": "<base64>",
      "mtu": 1280,
      "detour": "proxy",
      "peers": [
        {
          "address": "10.77.0.1",
          "port": 51821,
          "public_key": "<hub public key>",
          "allowed_ips": ["10.77.3.0/24"],
          "persistent_keepalive_interval": 25
        }
      ]
    }
  ],
  "route": {
    "rules": [
      {"ip_cidr": ["10.77.3.0/24"], "outbound": "space"}
    ]
  }
}
```

Why these values:

- `address` is a `/32`, not the `/24`. sing-box rewrites connections to the
  endpoint's own address to loopback. A `/24` there would rewrite every peer's
  address to loopback and break the LAN.
- `allowed_ips` is the whole space `/24`, so the endpoint accepts traffic from
  every member, not just the hub.
- `detour: "proxy"` is the full tunnel. The endpoint's UDP socket is created
  through the main outbound, which is what gets the handshake past DPI.
- `mtu: 1280` because the space packet is nested inside the main tunnel. It is
  the IPv6 minimum and leaves room for VLESS, TLS and the outer transport.
- `persistent_keepalive_interval: 25` keeps the hub's endpoint mapping alive
  through the server's NAT and through Xray's UDP association.

The rule sits at the top of `route.rules`, above the IPv6 reject and the DNS
hijack. It is an IP rule, so it does not need sniffing. In a voice-only session
the TUN's `route_address` is narrowed to the voice ranges; the space `/24` is
added to that list when a space is active, or the rule would never see a packet.

sing-box 1.14 endpoints have inbound and outbound behaviour. A peer connecting
to the local space address is rewritten to `127.0.0.1`, so a normal Windows
service listening on all interfaces answers without any port proxy. This is what
makes file sharing and RDP work.

- https://sing-box.sagernet.org/configuration/endpoint/wireguard
- https://singbox-internals.hidandelion.com/protocols/wireguard.html
- https://github.com/SagerNet/sing-box/issues/2236

### 3.4 Server hub

`qcspaces` is a kernel WireGuard interface, created by a oneshot systemd unit:

```
ip link add qcspaces type wireguard
ip addr add 10.77.0.1/16 dev qcspaces
wg set qcspaces listen-port 51821 private-key /etc/wireguard/qcspaces.key
ip link set qcspaces up
sysctl -w net.ipv4.ip_forward=1
iptables -A INPUT -p udp --dport 51821 -s 127.0.0.0/8 -j ACCEPT
iptables -A INPUT -p udp --dport 51821 -s 10.8.0.0/24 -j ACCEPT
iptables -A INPUT -p udp --dport 51821 -j DROP
iptables -A FORWARD -i qcspaces -o qcspaces -j DROP
```

The coordinator adds peers at runtime with `wg set qcspaces peer <pub>
allowed-ips 10.77.<n>.<x>/32`, and removes them on leave or kick. It restores
all peers from SQLite on start. For every space it also adds
`10.77.<n>.1/24` to the interface. That address is not cosmetic: without it the
kernel would source the hub's replies from `10.77.0.1`, which is outside the
client's `allowed_ips` (the space `/24`), and every reply would be dropped.
There is no `MASQUERADE`: space traffic never leaves the hub, so no NAT is
needed.

The hub keypair is generated once at install time. Its public half is returned
to clients in the join response.

### 3.5 Control plane

A single-file Python service, standard library only, SQLite for state. It runs
on the VPS beside the existing card API. TLS is terminated by the service itself
with the same certificate as the card API, or by a reverse proxy in front.

Default URL: `https://quotavpn.duckdns.org/spaces`. The desktop reads the stored
value first and `QC_SPACES_API` second, so a developer can point it at a local
coordinator over HTTP.

Entities:

- **Space**: id (8 characters, Crockford base32), name, subnet number, owner
  member id, invite hash, optional password hash, created timestamp.
- **Member**: id (UUID), space id, display name, WireGuard public key, address,
  token hash, last seen, created timestamp.

Endpoints:

| Method | Path | Auth | Body | Answer |
| --- | --- | --- | --- | --- |
| POST | `/v1/spaces` | none, rate limited | name, password, device_name, public_key | space, member, hub |
| POST | `/v1/spaces/join` | invite code | code, password, device_name, public_key | space, member, hub |
| GET | `/v1/spaces/{id}` | bearer | none | space, members, hub |
| POST | `/v1/spaces/{id}/heartbeat` | bearer | none | members, server time |
| POST | `/v1/spaces/{id}/leave` | bearer | none | ok |
| POST | `/v1/spaces/{id}/invite` | owner bearer | none | new invite code |
| POST | `/v1/spaces/{id}/kick` | owner bearer | member_id | members |
| DELETE | `/v1/spaces/{id}` | owner bearer | none | ok |

The create and join responses both contain:

```json
{
  "space": {"id": "K7M2Q9X4", "name": "Gaming", "subnet": "10.77.3.0/24"},
  "member": {
    "id": "uuid",
    "name": "Yousef-PC",
    "address": "10.77.3.7/32",
    "token": "64 hex characters"
  },
  "hub": {
    "public_key": "<base64>",
    "endpoint": "10.77.0.1",
    "port": 51821,
    "subnet": "10.77.3.0/24"
  }
}
```

The token is returned once, at create or join. The server stores only its
SHA-256 hash. The invite code is `qvs1:<space id>:<32 hex>`; the server stores
only a hash of the secret, salted with the space id. The password is optional
and stored with PBKDF2-HMAC-SHA256, 200,000 iterations. A space without a
password is protected by the invite code alone, the same trust model as a Radmin
network name and password.

Rate limits:

- Create: 5 per hour per source IP.
- Join: 10 failed attempts per hour per source IP, then a 15 minute cool-down.
- Heartbeat: 12 per minute per token.
- Maximum 253 members per space, 254 spaces per server.

A join attempt never reveals whether a space exists. It returns the same
`404 not found` for a bad id, a bad secret, a wrong password and a deleted
space.

### 3.6 Security model

- WireGuard encrypts every leg: client to hub, hub to client. The hub decrypts
  and re-encrypts, so this is hop-by-hop, not end-to-end between members. The
  server operator can see source and destination addresses and traffic volume.
  This must be stated in the UI's trust note. A later direct-path release can
  make member-to-member traffic end-to-end.
- The space private key and member token are generated on the client. The
  private key never leaves the PC. The token is sent only to the coordinator
  over TLS. Both live in `config.json` beside the card's WireGuard key for this
  slice; moving them into the DPAPI-sealed store is a phase-2 item.
- The hub is not reachable from the internet. The only way in is through the
  main tunnel or from the VPS itself.
- Members cannot reach another space. The per-space firewall rules are rebuilt
  on every membership change and verified by the coordinator's selftest.
- Leaving removes the peer from `wg` before deleting the member row. A kick
  does the same. A deleted space removes every peer and its firewall rule.
- The coordinator logs no payload and no IP-level traffic. It keeps the minimum
  needed to run the service: space rows, member rows, timestamps and a short
  audit line for joins, leaves and kicks. The audit line carries member id and
  action, never the token or invite code.
- The invite code is shown once and can be rotated by the owner. Rotating it
  does not disconnect existing members.
- A rogue member can still scan its own space's `/24` and reach services the
  other members expose. That is the same trust model as a physical LAN. The UI
  says so and recommends the Windows firewall profile for the space adapter.
  Per-member ACLs are a later feature.

### 3.7 Desktop UI

A new rail entry, `Spaces`, between Network and Speed. The screen is a two
column layout, like Network:

Left column: the list of spaces this PC belongs to. Each row shows the name,
the virtual address, the online count and a state dot. A footer action opens
Create or Join.

Right column: the selected space.

- Header: name, address with a copy button, an online pill and the hub state.
- Connect button: activates the space and, when the main VPN is off, starts it.
  If the main VPN is already on, the screen says "Reconnect to apply" instead of
  silently dropping the session.
- Invite block: the invite code with a copy button, the password state and a
  rotate action for the owner.
- Roster: member name, address, online dot, this-PC tag, owner tag. Clicking a
  row copies the address. A small hint explains that the other side must allow
  the port in Windows Firewall.
- Footer: Leave space for a member, Delete space for the owner. Both ask for a
  confirmation; removing the live space also stops the session.

Create dialog: space name, device name, optional password. Join dialog: invite
code, password, device name. No dropdowns. The existing dialog, input and press
patterns are reused.

The screen must work in the browser preview through `mock.ts`, with a demo space
and three members, so screenshots and layout checks do not need the engine.

All strings go through `src/lib/i18n/en.ts` and `ar.ts`. Arabic is formal MSA and
right-to-left text only; the layout does not mirror. No em dashes.

### 3.8 Phone

Phase 2. The phone already runs libbox with a TUN, and the sing-box endpoint
supports `detour`, so the same endpoint JSON can be merged into the phone config.
The UI is a fifth place, not a fifth tab: the bottom bar keeps Home, Speed, Apps
and Settings, and Spaces lives as a row in Settings that opens a full page. A
fifth tab at 360 px would make every label smaller; the 0.7.5 bar is already at
its comfortable width.

The phone work is gated on the desktop data plane working on the real VPS and on
the coordinator being deployed with a valid certificate.

## 4. What this release does not do

- No broadcast or multicast. LAN game discovery will not find a host by itself.
  Direct-IP joins work. This is the biggest gap against Radmin and it is
  deliberate: carrying Ethernet frames needs a TAP driver on Windows and a
  userspace L2 stack, which is a separate project.
- No direct peer-to-peer path. Every packet goes through the hub. A home user
  with a 35 Mbps uplink will see the hub's uplink as the ceiling when two peers
  in the same city transfer a file.
- No remote desktop. That is a later layer on top of the same roster: a Rust
  screen capture and input channel, rendezvous through the coordinator, and the
  space tunnel as the transport. RustDesk's model is the reference.
- No per-member ACLs, no DNS names, no subnet router, no site-to-site.
- No phone UI.

## 5. Phases

**Phase 1, this slice: control plane and desktop shell.**
- Core space model, key generation, invite parsing, endpoint JSON.
- Coordinator service with SQLite, create/join/list/heartbeat/leave.
- Desktop commands, persistence, Spaces screen, i18n, mock data.
- Config builder merges the endpoint and route rule; unit tests cover the shape.
- Dev coordinator runs on Windows without `wg`, so the join flow can be tested.

**Phase 2: real data plane.**
- Deploy the hub and coordinator to the VPS.
- Verify handshake through VLESS from Egypt, then through Hysteria2 and AWG.
- Verify SMB, RDP and HTTP between two PCs.
- Add `wg show` handshake data to the roster's online state.
- Move the space private key and token into a DPAPI-sealed store.

**Phase 3: phone.**
- Merge the endpoint into the libbox config.
- Spaces page under Settings.
- Verify on the physical phone on a mobile network.

**Phase 4: direct path and broadcast.**
- ICE/STUN hole punching with the hub as the fallback relay.
- Broadcast/multicast proxy for LAN game discovery, or a TAP-based L2 mode.
- Optional remote desktop on the same roster.

## 6. Test plan

Unit, on the build machine:

- `cargo test`: invite parsing, keypair shape, endpoint JSON, config merge, route
  rule order, voice-only route list.
- Coordinator `--selftest`: create, join, list, duplicate join, wrong password,
  leave, kick, delete, subnet reuse, cross-space firewall rules.
- UI `npm run build`: zero TypeScript errors.
- Preview at 1440x900 and 1100x760: create dialog, join dialog, empty state,
  selected space, roster, connect hint. Screenshot each.

Live, on the VPS and two Windows PCs:

- Create a space on PC A, join from PC B with the invite code.
- `ping 10.77.n.2` from B and `ping 10.77.n.3` from A.
- Open a share on A, reach `\\10.77.n.2\share` from B.
- RDP from B to A's space address.
- Run an HTTP server on A bound to all interfaces, open it from B.
- Confirm a member of another space cannot ping either address.
- Confirm `wg show qcspaces` lists both peers and the latest handshakes move.
- Disconnect the main VPN: the space endpoint goes down with it and the roster
  shows offline.
- Reconnect: the space recovers without a new invite.

## 7. Risks and open questions

1. **Xray UDP to a private address.** The live Xray config has a freedom
   outbound and no private-IP block, so `10.77.0.1:51821` should work. If the
   deployed config has a `geoip:private` block, the hub must be reached through
   the VPS's private address instead and the coordinator's hub endpoint must
   match. This is the first thing to check during deployment, on both a proxy
   transport and the WireGuard transport.
2. **Nested MTU.** 1280 is safe but not generous. Large SMB transfers may need
   TCP MSS clamping on the hub. The coordinator setup should add a clamp rule
   for `qcspaces`.
3. **Hub CPU.** A kernel WireGuard hub forwards at line rate for small groups.
   The VPS is an Oracle A1, so the ceiling is the network, not the CPU.
4. **Quota accounting.** Space traffic rides the main tunnel, so it spends the
   card's quota. The UI should say that before the user joins a large transfer.
5. **Certificate.** The coordinator needs a valid certificate for the API
   hostname. The card API already has one; reuse its files or put the
   coordinator behind the same reverse proxy.
6. **Abuse.** Anyone can create a space. The rate limit and the 253-member cap
   contain it, but a public deployment may need card-bound creation. That is a
   product decision, not a blocker for the first slice.
7. **Windows Firewall.** A peer connecting to a service on this PC needs the
   service allowed on the space adapter. The roster hint covers it; a one-click
   "allow file sharing" is a later nicety.
8. **Offline leave.** Leaving while the coordinator is unreachable removes the
   local record but cannot remove the hub peer. The peer entry and token stay
   until the owner kicks the member or deletes the space. A periodic expiry
   pass (for example, no heartbeat for 30 days) is a phase-2 item.

## 8. Deployment runbook

Server, as root:

```
install -m 755 embed/qc-spaces.py /usr/local/bin/qc-spaces
install -m 755 embed/qc-spaces-setup.sh /usr/local/bin/qc-spaces-setup
qc-spaces-setup
```

`qc-spaces-setup` installs `wireguard-tools`, writes the hub key, creates the
`qcspaces` interface unit, installs the coordinator unit, and starts both. The
coordinator database lives at `/var/lib/qc-spaces/spaces.db`. Open TCP 8445 in
the Oracle security list when the coordinator serves TLS, or keep the listener
on loopback behind a proxy, and open **UDP 8446** for the direct-path probe
rendezvous. The probe is a diagnostic only; it does not carry the data plane yet.
TLS certificate paths are read from `/etc/qc-spaces/env`:

```
QC_SPACES_CERT=/etc/letsencrypt/live/quotavpn.duckdns.org/fullchain.pem
QC_SPACES_KEY=/etc/letsencrypt/live/quotavpn.duckdns.org/privkey.pem
QC_SPACES_HOST=0.0.0.0
QC_SPACES_API_PORT=8445
```

For a first run without TLS, leave the env file absent. The setup script binds
the coordinator to `127.0.0.1:8445`; reach it through an SSH tunnel. Never
expose the plain HTTP listener to the internet. `QC_SPACES_HOST` is only read
when a certificate is configured.

Client development, on Windows:

```
python embed/qc-spaces.py --dev --port 8445 --db %TEMP%\spaces.db
set QC_SPACES_API=http://127.0.0.1:8445
```

The `--dev` flag skips `wg` calls and returns a fixed hub public key. It is for
the UI and the join flow only; it cannot carry traffic.

## 9. Tooling

Already present and used by this slice:

- Rust 2024, `ring`, `serde`, `uuid`, `reqwest`, `x25519-dalek`, `base64`.
- `sing-box` 1.14 fork (amnezia-box), already the desktop engine.
- Python 3 standard library for the coordinator. No pip packages.
- Tauri 2, React 19, Vite, Tailwind v4, Motion, Lucide, shadcn/ui.
- Playwright or chrome-devtools for the preview screenshots.

Server packages installed by the setup script:

- `wireguard-tools`
- `iptables` (already present)
- `python3` (already present)

No new MCP server is required. The existing `github`, `context7` and `tauri`
servers cover repository work, library docs and window verification. The
`tauri-mcp` skill is the way to drive the built Windows app; `browser` and
`playwright` cover the preview.

## 10. Acceptance for this slice

- A user can create a space from the desktop UI, see the invite code, copy it,
  and see the space in the list after a restart.
- A second install can join with the code and see both members with addresses.
- The generated sing-box config contains the `space` endpoint with a `/32`
  address, the `detour`, the `/24` allowed IPs, and a route rule above the IPv6
  reject.
- Leaving removes the member from the coordinator and from the local list.
- The desktop UI builds with zero TypeScript errors and the preview renders the
  empty, create, join and selected states.
- The Spaces screen can run a Direct path test. It uses the coordinator's UDP
  rendezvous and a Noise IK session over the same socket, and reports Direct or
  Relay with an RTT. It is a diagnostic and does not carry VPN traffic yet.
- `cargo test` is green.
- The phone build is untouched and still 0.7.5.

## 11. References

- Radmin VPN review and product description:
  https://www.security.org/vpn/radminvpn
- Radmin VPN security page:
  https://www.radmin-vpn.com/security/
- Radmin VPN Linux launcher, layer-2 and broadcast evidence:
  https://github.com/baptisterajaut/radmin-vpn-linux
- Tailscale control and data planes:
  https://tailscale.com/docs/concepts/control-data-planes
- Tailscale DERP:
  https://tailscale.com/kb/1232/derp-servers
- NetBird architecture:
  https://docs.netbird.io/about-netbird/how-it-works
- Tailscale, ZeroTier and NetBird comparison:
  https://tailscaleguide.com/posts/tailscale-vs-zerotier-vs-netbird-mesh-vpn-compared
- RustDesk self-hosting and relay flow:
  https://rustdesk.com/docs/en/self-host/
- sing-box WireGuard endpoint:
  https://sing-box.sagernet.org/configuration/endpoint/wireguard
- sing-box WireGuard internals, local address rewriting:
  https://singbox-internals.hidandelion.com/protocols/wireguard.html
- sing-box endpoint inbound discussion:
  https://github.com/SagerNet/sing-box/issues/2236
- WireGuard hub-and-spoke:
  https://jkrl.me/2026/05/11/hub-and-spoke-architecture.html

## 12. Research update: Radmin is direct-first

The first design put the hub on the data path for every packet. That is not how
Radmin behaves, and it is the reason the first slice cannot produce a 9 ms
Minecraft link. This section records the evidence and the corrected direction.

### 12.1 What Radmin actually does

- **Direct first.** Radmin tries to connect each pair of peers directly. The
  connection properties show `TCP/out` or `UDP/out` for a direct path and
  `TCP/relay` when the direct path failed. Project Fika documents the same
  check and warns that `TCP/relay` is much slower:
  https://wiki.project-fika.com/testing-connectivity/test-vpn-connectivity/ensuring-direct-connection
- **Relay fallback.** Famatech support calls Relay not an error but the
  connection type used when a direct connection is impossible, and lists NAT,
  firewalls, router settings and topology as the causes. Users in the same
  thread report direct ping around 50 ms and relayed ping over 400 ms:
  https://radmin-club.com/radmin-vpn/relay-connection-vs-direct-connection/
- **UDP hole punching.** Radmin listens on a random UDP port for peer data
  (users report the 58000 to 62000 range), has no UPnP or fixed-port option, and
  depends on a friendly NAT for a direct path. One side with a friendly NAT can
  be enough. Strict NAT, CGNAT and double NAT force the relay:
  https://radmin-club.com/radmin-vpn/nat-upnp-questions/
- **A coordination service and a relay fleet.** The client talks to Radmin
  servers for registration, network membership and peer discovery. Public
  support answers list several server and proxy addresses, and Famatech has
  rearranged its relay set by region. The data path is direct when possible; the
  server set is the fallback and the matchmaker:
  https://radmin-club.com/radmin-vpn/how-can-i-know-which-addresses-radmin-vpn-uses-to-communicate-with-its/
- **Layer 2, not layer 3.** The virtual adapter is an NDIS miniport on Windows.
  The Linux compatibility project replaces it with a TAP bridge and documents
  the Ethernet frames, MAC-based peer routing, the `26.0.0.0/8` route and the
  broadcast/multicast routes that make LAN games work:
  https://github.com/baptisterajaut/radmin-vpn-linux/blob/main/docs/architecture.md
- **Encryption claim.** Famatech states AES-256 end to end. The exact wire
  protocol is proprietary, but OpenRad interoperates with the official client,
  so its code is the best public description of the protocol shape:
  https://www.radmin-vpn.com/security/

The 9 ms case is therefore a direct path, most likely two peers in the same
country or city with compatible NAT. It is not a guarantee. A Radmin user behind
CGNAT or a strict firewall gets `TCP/relay` and a much higher ping.

### 12.2 GitHub implementations and references

| Repository | License | What it provides | Use to us |
| --- | --- | --- | --- |
| [gringoestrangeiro/openrad](https://github.com/gringoestrangeiro/openrad) | MIT | Rust reimplementation of Radmin VPN with direct TCP, reliable UDP, relay, TAP, Android client, self-hosted Go coordinator and Minecraft plugin | Closest working reference. MIT makes code reuse possible with attribution. |
| [Uss15/minilan-vpn](https://github.com/Uss15/minilan-vpn) | MIT | Node coordinator, Windows/Android clients, direct UDP hole punching, relay fallback, `100.77.0.0/16`, broadcast/multicast | Simpler protocol and relay-selection design. The wire format is documented in `PROTOCOL.md`. |
| [baptisterajaut/radmin-vpn-linux](https://github.com/baptisterajaut/radmin-vpn-linux) | GPL-3.0 | Wine/NDIS/TAP compatibility layer, Ethernet frame bridge, 26/8 and broadcast routes, port-reuse NAT shim | Best evidence of the adapter and L2 behavior. Do not copy GPL code into a non-GPL product. |
| [islamyus/radmin-vpn-mac](https://github.com/islamyus/radmin-vpn-mac) | GPL-3.0 | Native macOS client for Radmin networks, built on OpenRad | Evidence that the OpenRad engine and protocol are reusable across platforms. |
| [LinCastr/linmin-vpn](https://github.com/LinCastr/linmin-vpn) | GPL-3.0 | Native Linux front end for Radmin VPN | Frontend reference only. |

OpenRad's architecture is the most useful public source. It describes direct
TCP and UDP candidates, NAT endpoint discovery with two STUN-like servers,
hole punching, relay tickets, a four-second direct head start before relay
authentication, TAP Ethernet framing, broadcast policies and a Go coordination
server:
https://github.com/gringoestrangeiro/openrad/blob/main/docs/architecture.md

The code-level integration verdict is in `notes/OPENRAD-INTEGRATION.md`. OpenRad
is a full second VPN app with its own TAP driver, elevated service, identity and
Android VpnService, so it is not a source copy. The project decision is to build
the direct-first data plane inside QuotaVPN. OpenRad and MiniLAN are protocol
references, not a sidecar and not a second app.

MiniLAN is the smaller reference. It uses one UDP socket, X25519 and
ChaCha20-Poly1305, a coordinator that introduces peers, direct hole punching,
a relay when direct fails within about two seconds, and a relay fleet that
clients ping to choose the fastest node. Its `PROTOCOL.md` is short enough to
port:
https://github.com/Uss15/minilan-vpn/blob/main/PROTOCOL.md

### 12.3 What this changes in the spec

The current slice is not the final Radmin-equivalent design. It is:

- **Relay-only.** Every packet goes through the VPS. From Egypt to the Milan
  VPS the TCP RTT is about 46 to 50 ms, so a relayed Minecraft ping is around
  90 to 120 ms, not 9 ms.
- **Layer 3.** It can carry unicast IP but not Ethernet broadcast, so a
  Minecraft "Open to LAN" world will not appear in the LAN list.
- **Dependent on the main VPN.** Spaces only works while the QuotaVPN session
  is up, because the inner WireGuard endpoint uses `detour: "proxy"`.

Those are acceptable properties for a fallback and for file sharing, RDP and
HTTP. They are not acceptable if the acceptance test is "feels like Radmin in
Minecraft".

The corrected target is:

1. **Coordinator for signaling only.** Keep the existing space membership,
   invites and addresses. Add peer candidates and relay tickets.
2. **Direct path first.** UDP hole punching with STUN-like endpoint discovery,
   then direct UDP. If UDP is blocked, try direct TCP. Keep the hub as the
   fallback.
3. **DPI-safe direct transport.** A bare WireGuard handshake from Egypt is
   dropped. The direct path uses a Noise IK session over UDP
   (`Noise_IK_25519_ChaChaPoly_BLAKE2s`) instead of a WireGuard handshake. It is
   a different wire shape and does not reuse the main VLESS/Hy2 tunnel, which
   would go back through the VPS. Reusing the member's WireGuard static key as
   the Noise static key is a prototype shortcut; shipping should derive a
   separate Spaces path key.
4. **Layer 2 for Minecraft.** A TAP adapter and Ethernet framing, or a
   broadcast/multicast proxy on top of the L3 path. TAP is the general answer;
   a proxy is smaller but only covers games whose broadcast ports are known.
5. **Regional relay fleet.** If direct fails, one Milan relay is not enough.
   Clients should measure RTT to several relays and use the closest. A relay in
   Egypt or the Gulf would make the fallback tolerable.

The direct-path probe discovers the public UDP endpoint with a public STUN
server from the same socket it uses for the Noise handshake, then reports that
mapping to the coordinator over HTTPS. The coordinator's own UDP observer on
8446 is a fallback. A symmetric NAT can make the mapping useless even when a
different port might work; that is the expected relay case. The probe also
reuses the member's WireGuard static key for the Noise handshake, which is
acceptable for the prototype but should become a separate derived key before
shipping.

### 12.4 Revised phase order

The old order put the phone UI before the direct path. That is backwards if
low-ping Minecraft is the goal.

1. **Build the direct path in-house.** OpenRad, MiniLAN and the Radmin Linux
   shim are references only. The probe is done: it discovers the public UDP
   endpoint with STUN, reports it to the coordinator over HTTPS, establishes a
   Noise IK session (`Noise_IK_25519_ChaChaPoly_BLAKE2s`), and sends an
   encrypted echo. The Spaces screen shows Direct or Relay with an RTT.
2. **Packet forwarding.** The first data-plane slice is in the tree:
   `spaces_forward` carries IPv4 packets over the Noise session with explicit
   counters and a replay window, `spaces_direct` creates a second Wintun
   adapter at the member's `10.77.<n>.x/24`, and the Spaces screen has Start
   and Stop controls. The forwarding loop has a bidirectional loopback test
   over the encrypted path. The Wintun adapter has not been created on a live
   machine yet; the next test is two PCs with the QuotaVPN tunnel off. Keep the
   hub as the fallback. Broadcast/multicast for Minecraft LAN discovery is the
   next item after unicast IP works.
3. **Layer 2 and broadcast.** TAP on Windows, a merged VpnService on Android, or
   a broadcast proxy. Verify Minecraft LAN discovery.
4. **Phone UI.**
5. **Regional relays and remote desktop.**

The existing coordinator, invite model, Spaces screen and hub are still useful.
The hub becomes the fallback, and the coordinator gains peer candidates and
relay tickets. The UI does not need to be thrown away.

The official Radmin 2.1.4952.2 installer was inspected locally and is recorded
in `notes/RADMIN-INSPECTION.md`: signed by Famatech, packed Inno Setup 6.5.2,
Delphi/C++Builder x86. It was not installed while a live tunnel was up because
its `26.0.0.0/8` adapter and broadcast routes can change the routing table.

### 12.5 Honest expectation

Even after direct P2P, 9 ms is not guaranteed. It depends on both ISPs, NAT
type, distance and DPI. Radmin itself falls back to a relay for many users. The
goal should be "direct when the network allows it, relay when it does not", not
"always 9 ms".

### 12.6 Verified against the installed client

On 26 November 2026 the official client was installed and inspected live. The
full evidence is in `notes/RADMIN-WORKINGS.md`. The parts that change the
design:

- `RvControlSvc.exe` is a LocalSystem auto-start service and owns the entire
  network: the NDIS miniport, the server registration, every peer connection,
  the firewall rules and the GUI channel. `RvRvpnGui.exe` is a Qt view with no
  network connections; it talks to the service over
  `\\.\pipe\RadminVpnGuiChannel`.
- The service registers within about five seconds of starting, assigns
  `26.x.y.z/8` and `fdfd::<ipv4 in hex>`, and stays connected with the window
  closed. The UI is optional.
- The peer paths are `TCP/out`, `TCP/in`, `UDP/nat` and `Relay`. The ROL client
  contains ENet reliable UDP, TCP connectors, NAT "pen wave" hole punching, a
  crypto channel and DNS over HTTPS for server discovery.
- The service keeps a graph of networks, nodes and reachability, monitors the
  adapter and the Windows Filtering Platform, reinstalls the adapter if it
  disappears, and adds its own firewall rules.

The Spaces gap is ownership, not protocol. The direct engine currently starts
from the Spaces screen and stops when the screen unmounts. Radmin's engine
starts with the machine and runs until it stops. The first change after 0.7.8 is
therefore a Spaces supervisor: start the direct engine from the app, poll the
coordinator for an online peer, keep the session alive while the space is
active, and let the UI be a view of that state. TCP fallback, a coordinator
relay and multi-peer forwarding follow in that order.
