# QuotaVPN 0.7.12 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## TCP hole punching, not a plain listener

The 0.7.11 port test settled it: from the VPS, both `51393` (Radmin's port)
and `51394` (ours) were closed. Radmin's live TCP connection was therefore
hole-punched from the same local port, not accepted on a port the router
already forwards. Both sides send first, which opens the router mapping for the
other address; then one SYN gets through.

This build does that:

- A TCP STUN probe from the listener port (`stun.nextcloud.com:3478`) returns
  the real mapped `ip:port`, not the guessed local port. With the listener and
  the probe both on `51394`, the router answered `197.58.209.156:51394`.
- The owner punches from `51394` to the joining member's mapped endpoint, then
  accepts on `51394`.
- The joining member connects from its own `51394` to the owner's mapped
  endpoint, retrying for 12 seconds.
- The Noise IK handshake runs over the first stream that gets through, and the
  framed TCP transport carries the space as before.

This is the same symmetric port reuse Radmin uses. The previous build guessed
the local port and held a plain TCP connection to the coordinator, which did
not keep a mapping the peer could use.

## Test

Both PCs must run 0.7.12. The invite and the space do not change.

Installer:
`https://github.com/YousefMohiey/QuotaVPN/releases/download/v0.7.12/QuotaVPN_0.7.12_x64-setup.exe`

1. Install on this PC.
2. Send the same link to your friend and install over 0.7.11. The space
   `verh` and the coordinator persist.
3. Open both apps and leave them. The supervisors punch and connect.
4. Expect `Connected directly` with `10.77.2.2` and `10.77.2.3`.

## If it still fails

Symmetric NAT or a firewall that blocks the punched port will still stop it.
The only reliable fallback then is a relay through the public coordinator. The
direct attempt is now complete; no more guessed ports.

## Limits

- One peer per direct session.
- Broadcast and multicast are not carried; join Minecraft by address.
- The installer is unsigned and will trigger SmartScreen.
