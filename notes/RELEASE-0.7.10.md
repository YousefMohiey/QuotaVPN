# QuotaVPN 0.7.10 experimental

Desktop-only test build. The phone remains on 0.7.5. Stable stays on 0.7.5.

## Zero setup for the other PC

The invite now carries the coordinator address. The other PC installs the app,
opens **Join a space**, pastes one code, and nothing else. No `setx`, no
environment variable, no terminal.

- The owner's invite is `qvs2:<coordinator>:<code>`. The app wraps it when it
  shows the Invite panel.
- The joining app reads the coordinator out of the code, saves it, and uses it
  for the join and for every later request.
- A plain `qvs1` code still works for an older install pointed at the same
  coordinator.
- The Spaces header shows the current coordinator and lets you change it in the
  app. The value is saved in `config.json`.
- An existing `QC_SPACES_API` is adopted once at startup, so the old test setup
  keeps working without the variable.

## Public coordinator

The coordinator now runs on the existing VPS at:

```
https://quotavpn.duckdns.org/spaces
```

It is a `qc-spaces.service` on `127.0.0.1:8445`, reached through the same Xray
TLS fallback and nginx path that already serves `/qa-api/`. No Oracle security
list change, no new open port, and no VPN service was restarted. The create and
join API was verified through the public HTTPS URL.

This is the Radmin model: a public introduction server, then a direct data
path. Radmin's own live session was inspected during this work and its data
path was direct TCP, not relay.

## The space stays on

From 0.7.9: the direct engine belongs to the app, not the Spaces screen. The
supervisor polls the coordinator, starts the direct path when a member is
online, and keeps it alive when the window is hidden or another page is open.
**Stop direct LAN** pauses it. **Connect now** resumes it.

## Test flow

1. Install 0.7.10 on both PCs.
2. On this PC, create a space. The old space from the LAN coordinator will not
   exist on the public one, so create a fresh space for this test.
3. Copy the invite from the Invite panel and send it to your friend.
4. On the friend's PC: install, open Spaces, **Join a space**, paste the code,
   name the device, join. Nothing else.
5. Leave both apps open. Both cards should reach **Connected directly** within
   20 to 30 seconds.
6. Ping the address from the roster, then use it for Minecraft.

## Limits

- One peer per direct session.
- The data path is still UDP with STUN and Noise. If the friend's network drops
  UDP, the automatic attempt will not connect. Radmin used TCP in the live
  session that was inspected, so a TCP path is the next change if this build
  fails at Connecting.
- Broadcast and multicast are not carried; join Minecraft by address.
- The Wintun adapter is still untested against a live peer.
- The installer is unsigned and will trigger SmartScreen.
