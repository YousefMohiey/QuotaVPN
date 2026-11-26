# QuotaVPN 0.7.14 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## The 10048 collision

0.7.13 opened three sockets on the same local port: the STUN mapping, the
listener, and the punch/session. Windows allows the first two and rejects the
third with:

```
tcp connect: Only one usage of each socket address
(protocol/network address/port) is normally permitted. (os error 10048)
```

Radmin keeps a listener and reuses the port for the peer, but it does not keep
a third STUN socket alive. This build matches that:

- The STUN mapping is closed before the punch on both sides.
- The owner keeps the listener and the punch.
- The joining side drops its listener before connecting, so the owner's punch
  cannot sit in its backlog and occupy the port.
- The session socket is the only socket the joining side holds on `51394`.

The port that was advertised to the coordinator is still `51394`; the punch
from that port recreates the same mapping.

## Test

Both PCs must run 0.7.14. The space and invite do not change.

Installer:
`https://github.com/YousefMohiey/QuotaVPN/releases/download/v0.7.14/QuotaVPN_0.7.14_x64-setup.exe`

1. Install on this PC.
2. Send the same link to your friend and install over 0.7.13.
3. Leave both apps open. Watch the top card, not the old Direct path test.
4. Expect `Connected directly`; ping only after that.

## If it still fails

If the log now shows `tcp connect timed out` instead of 10048, the port
collision is gone and the router is refusing the punched connection. That is
the point where only the relay will work. Send the exact log line.

## Limits

- One peer per direct session.
- Broadcast and multicast are not carried; join Minecraft by address.
- The installer is unsigned and will trigger SmartScreen.
