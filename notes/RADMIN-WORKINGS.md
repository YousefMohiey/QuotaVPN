# How Radmin VPN works

Findings from the installed client, version 2.1.4952.2, on 26 November 2026.
The installer signature is valid (Famatech Corp., DigiCert) and its SHA-256 is
`717A87141FD703BB58BB730D3282E31055BF4EA9C319177ADE9B793974A7D5A4`.

Tools used: live process and network inspection, the installed INF, the service
log, MSVC RTTI names in the binaries, and Craft Anything 0.6.1 (`triage`,
`native-functions-list`) on the service, the ROL client and the driver.

## Components

- `RvControlSvc.exe` is the whole VPN. It is a LocalSystem auto-start service
  (`"C:\Program Files (x86)\Radmin VPN\RvControlSvc.exe" /service`). It owns the
  virtual adapter, the server registration, every peer connection, the firewall
  rules, and the GUI channel.
- `RvRvpnGui.exe` is a Qt 5 window. It has no network connections at all. It
  talks to the service over the named pipe `\\.\pipe\RadminVpnGuiChannel`. The
  command line is `/show`.
- `RvNetMP60.sys` is a 65 KB x64 NDIS 6 miniport. Its INF is `oem38.inf`, driver
  date 09/08/2026, hardware ID
  `{B06D84D1-AF78-41EC-A5B9-3CCE676528B2}\RvNetMP60`. It imports only
  `NDIS.SYS` and `ntoskrnl.exe`, is `NCF_VIRTUAL`, and presents an 802.3
  Ethernet media type. The service configures it with its own IOCTLs
  (`ioctl/setproto`, `ioctl/setmac` in the service strings).
- `RvROLClient.dll` is the peer transport. It brings ENet-style reliable UDP,
  TCP connectors, NAT hole punching, a crypto channel and DNS over HTTPS.
- `RvUESClient.dll` supplies NAT type detection and server error phrases.
- `RvFwHelper.exe` manages firewall rules. `Radmin.exe` is the separate Radmin
  Viewer, not part of the VPN.

## Address and routes

The adapter is named `Radmin VPN`, MAC `02-50-56-D8-74-A9`, and the service
assigns `26.108.10.101/8` with IPv6 `fdfd::1a6c:a65`. The IPv6 address is the
IPv4 address in hex after `fdfd::`.

Routes installed by the service:

```
26.0.0.0/8          on-link   Radmin VPN   metric 256
224.0.0.0/4         on-link   Radmin VPN   metric 256
255.255.255.255/32  on-link   Radmin VPN   metric 256
26.108.10.101/32    on-link   Radmin VPN   metric 256
0.0.0.0/0           26.0.0.1  Radmin VPN   metric 9256
```

The backup default route through `26.0.0.1` has a very high metric. It is not
used while the physical default exists, but it keeps a path through the virtual
gateway if the physical one disappears.

The service log records the server assignment:

```
Registered as #195067111, 26.108.10.101/fdfd::1a6c:a65
Virtual network adapter ready
Connected to server (15)
```

`#195067111` is the RID, the account identity in the Radmin network. The
registry stores it as `RID` (QWORD) under
`HKLM\SOFTWARE\WOW6432Node\Famatech\RadminVPN\1.0`, together with `IPv4`
(`0x1a6c0a65`), `IPv6` (`FDFD::1A6C:0A65`), `Alias`, `ServerAddress`,
`ProxyServers` and `FailServers`.

## Control plane

The service keeps a TCP connection to `198.244.203.247:17301`, which resolves to
`ns3237531.ip-198-244-203.eu` (OVH). The local port is `51393`, and the same port
is also a listener on the LAN address:

```
Listen       192.168.1.52:51393
Established  192.168.1.52:51393 -> 198.244.203.247:17301
```

Using one port for the outbound control connection and inbound peer connections
is deliberate: peers and the server see a stable port, which helps NAT mapping.

The service rotates through `FailServers` and `ProxyServers` from the registry
when the chosen server does not answer. RTTI names confirm the machinery:
`CFailServerList`, `CProxyServerList`, `CServerSettings`, `CPacket_KeepAlive`,
`CPacket_KeepAliveAck`, `CPacket_RadminPort`, `CRegister`, `CRadminListener`.

`RvROLClient.dll` contains `https://%hs/dns-query`, so server discovery and
verification also use DNS over HTTPS. It imports `DNSAPI.dll`, `NETAPI32.dll`,
`libcurl.dll` and `RvUESClient.dll`.

The service connects and registers within about five seconds of starting, before
and without the GUI. The GUI can be closed and the network stays up.

## Data plane

- One UDP socket is bound to the virtual address, `26.108.10.101:58698`. Packets
  written to the adapter are read there and carried to peers; peer packets are
  written back to the adapter.
- The TCP listener on `51393` accepts peers that cannot receive UDP.
- The ROL client names four path types, and the GUI strings use the same words:
  `TCP/out`, `TCP/in`, `UDP/nat`, `Relay`.
- RTTI names in `RvROLClient.dll` show how the paths are built:
  `ENETConnect` (reliable UDP), `NTrsConnect` (transport), `NTcpConnect`
  (legacy and universal TCP), `CEnetChannel`, `CPenSocket`,
  `CPenWaveSenderReceiver`, `SNatPenPacket`, `SNatPenWave`,
  `CTcpNatInfoPacket`, `VCCryptoChannel`, `IRosProtocol`.
- `SNatPenPacket` and `SNatPenWave` are the UDP hole punching probes. The
  client sends waves of packets to open the NAT mapping on both sides.
- `UESC_GetTcpNatType` classifies the NAT from the TCP side.
- `TcpRelay` in the service and the `Relay` path in the GUI are the fallback
  when both direct paths fail.
- Traffic is carried inside a crypto channel (`VCCryptoChannel`). The channel is
  authenticated per session; the protocol is proprietary and was not decrypted.

## Service internals

MSVC RTTI survived the strip and names the object graph:

- Networks and nodes: `rvpn::CNetworksMap`, `CNetwork`, `CNodesMap`,
  `CNode`, `CUiNetwork`, `CUiNode`, `CNodeConnectableStatus`,
  `SConflictedIpAddress`, `CNetEntry`, `CNodeMapEntry`.
- Adapter and firewall: `CMonitorAdapter`, `CMonitorWFP`,
  `CAdapterReinstalledServiceCommand`, `CAddFirewallExceptionServiceCommand`,
  `SFirewallRule`.
- GUI sessions: `CGui`, `CGuiClient`, `CGuiClientId`,
  `CUniqueGuiRequestId`.
- Work items: `CRegister`, `CRadminListener`, `CInstallRadminShim`,
  `CProxyServerList`, `CFailServerList`, `CServerSettings`, `CTaskCompleted_Register`.
- Chat and presence objects also exist (`rvpn::chat::*`), so the service is the
  single owner of the network graph, presence and messages.

The service monitors the adapter and the Windows Filtering Platform, reinstalls
the adapter if it disappears, and adds its firewall exceptions. Nothing in the
UI does that work.

## Behaviour, in plain terms

1. The network is up as soon as the service starts. The UI is optional.
2. The UI is a client of the service over a named pipe. It owns no connections
   and no adapter.
3. One adapter serves the whole machine and many peers. The service keeps a
   graph of networks, nodes and their reachability.
4. A peer is reached by UDP hole punching first, then by TCP out or in, then by
   the relay. The path is chosen per peer.
5. The service repairs the adapter and the firewall rules itself.
6. Radmin is its own network stack. It never needs the QuotaVPN main tunnel, and
   it does not ride another VPN.

## What this means for QuotaVPN Spaces

The 0.7.8 direct path has the right primitives: a coordinator, a STUN-discovered
UDP endpoint, a Noise session, an explicit counter layer, and a Wintun adapter at
`10.77.<space>.x/24`. What it does not have is Radmin's ownership model.

Current gaps:

- The engine starts from the Spaces screen and stops when that screen unmounts
  or the app exits. Radmin's engine runs from boot.
- One peer per session. Radmin keeps a peer graph.
- UDP only, with no TCP out, TCP in, or relay fallback. Radmin tries all four.
- The relay is the QuotaVPN hub and needs the main VPN. Radmin's relay is part
  of its own service.
- No adapter or firewall self-repair. A missing Wintun adapter is an error.

Order of work that closes the gap:

1. **Spaces supervisor.** Start the direct engine from the app, not the screen.
   On launch, if a space is active, poll the coordinator, pick an online peer
   and keep the engine alive until the space changes or the app exits. The UI
   reads status; it no longer owns the session.
2. **TCP fallback.** Listen on a stable port and connect out to the peer's
   advertised TCP endpoint, carrying the same Noise session with a length
   prefix. This covers networks that drop UDP.
3. **Relay fallback.** Add a packet relay to the coordinator for peers where
   both direct paths fail. The payload stays encrypted end to end.
4. **Multi-peer.** A peer table keyed by the space address, and a forward loop
   that chooses the peer from the destination `10.77.x.y` instead of a single
   session.
5. **Self-repair.** Ensure the adapter and its firewall rule on every start,
   recreate them if missing, and keep the address and metric.

The first item is the one that changes the feel from "a test screen" to
"a network that is on". It does not need a new driver or a new protocol.
