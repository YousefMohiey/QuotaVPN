# QuotaVPN 0.7.6-exp.3

One experimental build for the 0.7.6 line, replacing 0.7.6-exp.2. Stable
remains 0.7.5, and the phone remains 0.7.5. A future stable `0.7.6` sorts
above it.

## Direct TCP fix

The joiner used to open its TCP punch, then hold that socket while trying to
bind the listener to the same local port. When the punch succeeded, the
listener bind failed with `10048` (address in use) and the punch socket was
dropped, which closed the connection the owner was already handshaking on.
The owner then reported:

`direct send: ... the socket is not connected ... (os error 10057)`

The joiner now uses the punched connection when it succeeds. It only creates
the listener when the punch fails. This removes the `10048` / `10057` cycle.
TCP_NODELAY is set on the direct stream so small game packets are not held by
Nagle.

The build logs one of these lines while starting:

`app: spaces direct: joiner used the punched TCP path`

`app: spaces direct: joiner is listening for the TCP connection`

If the direct pair still fails, send the last 80 lines of:

`%LOCALAPPDATA%\quotavpn\engine\singbox.log`

from both PCs. The `spaces direct` and `spaces supervisor` lines identify the
exact stage that failed.

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
likely block the unsigned app again. A build signed with a certificate from a
CA in the Microsoft Trusted Root Program is the only way to run with Smart App
Control on.

## Friend test

Both PCs install this build. The space and invite do not change.

1. Install on both PCs.
2. You are the owner and connect out; the friend joins and listens.
3. Watch the top status band, not the old Direct path test.
4. If it stays on Ready, send both logs before trying again.

## Notes

- `0.7.6-exp.2` is deleted. This is the only experimental release.
- A 0.7.5 install on the experimental channel sees `0.7.6-exp.3` as newer.
- An exp.2 install on the experimental channel also sees exp.3 as newer.
- Switching back to Stable does not downgrade. On `0.7.6-exp.3` the stable feed
  is 0.7.5, which is lower. To return now, run the 0.7.5 installer. The next
  stable release, 0.7.6, sorts above exp.3 and will be offered normally.
- Installer SHA-256 is printed in the release notes after the build.
- The installer is unsigned and will trigger SmartScreen.
