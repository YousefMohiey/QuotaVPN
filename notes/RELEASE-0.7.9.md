# QuotaVPN 0.7.9 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## The space stays on

This is the Radmin ownership change. The direct engine no longer belongs to the
Spaces screen.

- A space that is active connects by itself. The app polls the coordinator,
  picks the first online member and starts the direct path. Leave the Spaces
  screen, hide the window to the tray, or open another page: the engine keeps
  running.
- The card shows **Connecting**, then **Connected directly** with the
  `10.77.x.x` address. No main VPN and no per-session button are needed.
- **Stop direct LAN** pauses the automatic connection for this run. **Connect
  now** resumes it immediately.
- Switching to another space stops the old engine and starts the new one.
- The app adds its own inbound UDP and TCP firewall rules for every network
  profile, the way Radmin's service adds its own exceptions.
- The coordinator rendezvous window is 18 seconds instead of 9, so the two
  automatic attempts overlap even when the apps start a few seconds apart.

## Testing with a friend

Both PCs must run this build and open the same space. The coordinator is still
the signaling path. On the friend's PC, point the app at the coordinator before
launching it:

```
setx QC_SPACES_API "http://192.168.1.52:8445"
```

Close and reopen the app. If the app still reports the old address, sign out and
back in, or launch it from a terminal with the variable set:

```
$env:QC_SPACES_API = "http://192.168.1.52:8445"
& "C:\Program Files\QuotaVPN\QuotaVPN.exe"
```

- Same LAN: use the coordinator on this PC at `http://192.168.1.52:8445`. It is
  running and the firewall rule allows the local subnet.
- Different networks: the coordinator must be reachable by both. This build does
  not include a relay or a public coordinator, and no tunnel is set up. Without
  a public coordinator the two apps cannot find each other.

The direct data path itself is peer to peer. The coordinator only exchanges
endpoints and public keys.

## What to expect

1. Install 0.7.9 on both PCs.
2. Create or join the same space on both. The invite code is in the space's
   Invite panel.
3. Leave the cards alone for 20 to 30 seconds. Each side should go through
   **Connecting** and reach **Connected directly**.
4. Ping the other address, then host Minecraft and join by `10.77.x.y:25565`.

If it stays on Waiting for a member, the other side is not registered as online
with the coordinator. If it stays on Connecting, the peer is online but the UDP
rendezvous did not complete; the app log records the reason.

## Limits

- One peer per direct session. Multi-peer is the next step.
- UDP only. TCP fallback and a coordinator relay are not in this build.
- Broadcast and multicast are not carried yet, so Minecraft LAN discovery does
  not work; join by address.
- The Wintun adapter has still not been exercised on two live PCs. This is the
  first build that will create it automatically, so report the error text if it
  fails.
- The installer is unsigned and will trigger SmartScreen.
