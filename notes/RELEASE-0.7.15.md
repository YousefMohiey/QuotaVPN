# QuotaVPN 0.7.15 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## The listener must be last

0.7.14 closed the STUN socket and freed the port, but the owner still created
its listener **before** the punch. On Windows the punch then took the inbound
connection, the listener never accepted, and the joining side saw:

```
direct receive: ... socket is not connected ... (os error 10057)
```

The order is now:

1. STUN from `51394`, discover the public endpoint, close the STUN socket.
2. Owner: punch from `51394` to the peer.
3. Owner: create the listener on `51394`, after the punch.
4. Owner: accept.
5. Joining side: drop its listener, connect from `51394`.

The listener is the last socket bound to the port, so it owns new inbound
connections. This is the same local ordering Radmin's service relies on.

This order was tested locally before this build:

```
stun_closed
punch_connected
listener_bound
listener_accepted ('127.0.0.1', 60612)
```

A separate client connected to `51394` and the listener accepted it while the
punch connection was still open.

## Test

Both PCs must run 0.7.15. The space and invite do not change.

Installer:
`https://github.com/YousefMohiey/QuotaVPN/releases/download/v0.7.15/QuotaVPN_0.7.15_x64-setup.exe`

1. Install on this PC.
2. Send the same link to your friend and install over 0.7.14.
3. Leave both apps open. Watch the top card, not the Direct path test.
4. Expect `Connected directly`; ping only after that.

## If it still fails

Send the exact `spaces supervisor` line from
`%LOCALAPPDATA%\quotavpn\engine\singbox.log` on both PCs. If the order is
correct and the routers still refuse the punched connection, the relay is the
only remaining option.

## Limits

- One peer per direct session.
- Broadcast and multicast are not carried; join Minecraft by address.
- The installer is unsigned and will trigger SmartScreen.
