# QuotaVPN 0.7.11 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## Direct TCP, the Radmin path

The live Radmin session was inspected while it was connected:

- this PC `192.168.1.52:51393` to the friend `154.177.91.38:62527`
- service log: `Connected to 190600030/'LOQ' via TCP/incoming`
- 12 to 23 ms direct, no relay

That is a direct TCP connection, not UDP. The 0.7.10 UDP handshake timed out
between the same two PCs, so this build replaces the data path with TCP.

What changed:

- Each member opens a stable TCP listener on port `51394` (the next free port
  if it is taken) and keeps a helper connection to the coordinator from that
  same local port. The helper keeps the NAT mapping open, which is what lets
  the other PC connect in. Radmin does the same with its control port.
- The public TCP endpoint (`public-ip:port`) is reported to the coordinator and
  handed to the peer.
- The space owner listens. The other member connects out. The Noise IK
  handshake and the encrypted frame layer run over the TCP stream with a
  two-byte length prefix.
- The Wintun adapter and the forwarding loop are unchanged; only the carrier
  changed from UDP to TCP.

The coordinator now stores and returns each member's TCP endpoint. It was
updated on the VPS without touching Oracle, Xray, nginx paths, or the VPN
services.

## Test

The invite code does not change. Both PCs must install 0.7.11.

1. Install 0.7.11 on this PC. The existing space `test` stays.
2. Send the same installer to your friend:
   `https://github.com/YousefMohiey/QuotaVPN/releases/download/v0.7.11/QuotaVPN_0.7.11_x64-setup.exe`
3. Both apps open. The supervisors start the TCP attempt automatically.
4. Expect `Connecting`, then `Connected directly` with `10.77.2.2` and
   `10.77.2.3`.
5. Ping the other address, then use it for Minecraft.

## If it still fails

The owner's router must keep the helper port mapping. If the owner's network
rewrites the source port, the advertised endpoint is wrong and the friend's
connect will time out. The next fallback is a relay through the public
coordinator, which removes the NAT dependency but adds the VPS round trip.
The app log records the exact TCP error under `spaces supervisor`.

## Limits

- One peer per direct session.
- The owner must be reachable on its advertised TCP port for this first TCP
  build.
- Broadcast and multicast are not carried; join Minecraft by address.
- The Wintun adapter is still untested against a live peer.
- The installer is unsigned and will trigger SmartScreen.
