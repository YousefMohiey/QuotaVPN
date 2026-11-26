# QuotaVPN 0.7.6-exp.4

One experimental build for the 0.7.6 line, replacing 0.7.6-exp.3. Stable
remains 0.7.5, and the phone remains 0.7.5. A future stable `0.7.6` sorts
above it.

## Verified against the VPS in the joiner role

The earlier VPS test had this PC as the owner and the VPS as the listener, so
it never ran the Windows joiner code the friend was failing in. This build was
tested in the opposite direction:

- The VPS created the space and acted as owner.
- This PC joined and acted as the joiner.
- The app logged `joiner used the punched TCP path`.
- The Noise handshake completed.
- `ping` to the VPS member returned 4 of 4 at 49 to 55 ms.
- The test was repeated with `wintun.dll` deleted first. The app downloaded
  the pinned driver, created the adapter, and ping returned 4 of 4 again.

## Direct TCP fix

The joiner used to open its TCP punch, then hold that socket while trying to
bind the listener to the same local port. When the punch succeeded, the
listener bind failed with `10048` (address in use) and the punch socket was
dropped, which closed the connection the owner was already handshaking on.
The owner then reported:

`direct send: ... the socket is not connected ... (os error 10057)`

The joiner now uses the punched connection when it succeeds. It only creates
the listener when the punch fails. TCP_NODELAY is set on the direct stream.

## Wintun on a fresh install

Spaces used to require `wintun.dll` from a previous main VPN connection. A PC
that installed QuotaVPN and went straight to Spaces had no driver, so the TCP
handshake succeeded, the adapter failed, and the other side saw a closed
connection. Spaces now downloads only the pinned Wintun driver when it is
missing, verifies its SHA-256, and then creates the adapter. The full sing-box
engine is not needed.

The start path also checks that the app is elevated and reports that plainly
instead of failing after the handshake.

## Config protection

- Config saves go through a temporary file and keep the last good
  `config.json.bak`. A force-kill cannot leave a half-written config that
  resets the cards and spaces.
- A UTF-8 BOM is tolerated when reading the config.
- `QC_SPACES_API` is no longer adopted into the config. A stale LAN value from
  an old test cannot overwrite the public coordinator again.

## Smart App Control

The installer and the app are unsigned. Windows 11 Smart App Control blocks
unsigned apps, and it has no per-app allow.

If Windows says "Smart App Control has blocked this app":

1. Run Windows Update first. Current Windows 11 can switch Smart App Control
   off and back on. Older builds may need a reset before it can be enabled
   again.
2. Open Windows Security, then App & browser control, then Smart App Control
   settings, and select Off.
3. Run the installer. If SmartScreen still warns, choose More info, then Run
   anyway.
4. Check the installer SHA-256 before running it.

Leave Smart App Control off while using this build. Turning it back on will
likely block the unsigned app again.

## Friend test

Both PCs install this build. The friend is the joiner, so their installer is
the one that matters most.

1. Install on both PCs.
2. You are the owner and connect out; the friend joins and listens.
3. Watch the top status band, not the old Direct path test.
4. If it stays on Ready, send the last 80 lines of
   `%LOCALAPPDATA%\quotavpn\engine\singbox.log` from both PCs.

## Notes

- `0.7.6-exp.3` is deleted. This is the only experimental release.
- A 0.7.5 install on the experimental channel sees `0.7.6-exp.4` as newer.
- An exp.3 install on the experimental channel also sees exp.4 as newer.
- Switching back to Stable does not downgrade. On `0.7.6-exp.4` the stable feed
  is 0.7.5, which is lower. To return now, run the 0.7.5 installer. The next
  stable release, 0.7.6, sorts above exp.4 and will be offered normally.
- Installer SHA-256:
  `ABB61F87E5D3F9FE5DEEACC0633D2B3499B6887A72F5B971F38B6BF98B1A636C`
- The installer is unsigned and will trigger SmartScreen.
