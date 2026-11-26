# QuotaVPN 0.7.13 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## The listener is back

0.7.12 bound the TCP STUN helper **after** the listener. On Windows that takes
the port: `Get-NetTCPConnection` showed `Bound` and the STUN connection, but no
`Listen`. The peer had nothing to connect to.

This build binds in the Radmin order:

1. The TCP STUN connection from the chosen local port (`51394`).
2. The listener on the same port.
3. The punch/session from the same port.

The helper holds the STUN stream instead of reconnecting, because a reconnect
would bind the port again and take `Listen` away a second time.

The owner punches to the joining member's STUN endpoint, then accepts. The
joining member connects from its own listener port and retries. The Noise
handshake runs over the surviving TCP stream.

## Test

Both PCs must run 0.7.13. The invite and the space do not change.

Installer:
`https://github.com/YousefMohiey/QuotaVPN/releases/download/v0.7.13/QuotaVPN_0.7.13_x64-setup.exe`

1. Install on this PC.
2. Send the same link to your friend and install over 0.7.12. The space and the
   coordinator persist.
3. Open both apps. Do not use the old **Direct path** test; that is the UDP
   diagnostic and will always time out on this network. Watch the top card for
   `Connected directly`.
4. Ping the other `10.77.x.x` address once the top card says connected.

## If it still fails

The `Listen` row is the first thing to check. If it is missing, the port was
taken again. If it is present and the punch still cannot pass, the router is
symmetric and only the relay will work.

## Limits

- One peer per direct session.
- Broadcast and multicast are not carried; join Minecraft by address.
- The installer is unsigned and will trigger SmartScreen.
