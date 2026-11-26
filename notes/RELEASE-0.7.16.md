# QuotaVPN 0.7.16 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## The roles were backwards

The live Radmin session was:

```
Established 192.168.1.52:51393 -> 154.177.91.38:62527
```

Port `51393` was closed when tested from the VPS, so this PC initiated the
connection to the friend and the friend's router accepted it. This PC's router
blocks inbound. The owner must connect out, and the joiner must listen.

0.7.16 swaps the roles:

- **Owner connects out** from `51394` to the joiner's mapped endpoint.
- **Joiner listens**: STUN from `51394`, close it, punch the owner, create the
  listener on `51394` last, accept.

## Verified end to end against a VPS peer

The friend was offline, so a real peer was run on the VPS at `10.77.2.3`:

- It joined the space, listened on the public `8444` port, and reported its
  endpoint.
- This PC connected out to `80.225.89.162:8444` from `51394`.
- The Noise IK handshake completed (`handshake True`, `keys ready`).
- The app created `QuotaSpace8007` and logged
  `spaces supervisor: direct 10.77.2.2/32 with vps-peer`.
- `ping 10.77.2.3` returned 4 of 4 replies, 97 to 137 ms.
- The VPS peer logged four echo frames.

This is the full app path: TCP role, Noise, Wintun adapter, forwarding, and
ping over a remote public peer.

## Friend test

Both PCs must run 0.7.16. The space and invite do not change.

Installer:
`https://github.com/YousefMohiey/QuotaVPN/releases/download/v0.7.16/QuotaVPN_0.7.16_x64-setup.exe`

1. Install on this PC.
2. Send the same link to your friend and install over their build.
3. Create one fresh space. You are the owner; the friend joins with the invite.
4. Leave both open. You connect out; the friend listens.
5. Watch the top card, not the old Direct path test.

If the friend's router refuses the outbound connection, the remaining fallback
is a relay through the coordinator.

## Limits

- One peer per direct session.
- Broadcast and multicast are not carried; join Minecraft by address.
- The installer is unsigned and will trigger SmartScreen.
