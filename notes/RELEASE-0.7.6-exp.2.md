# QuotaVPN 0.7.6-exp.2

One experimental build for the 0.7.6 line, replacing 0.7.6-exp.1. Stable
remains 0.7.5, and the phone remains 0.7.5. A future stable `0.7.6` sorts
above it.

## Coordinator default

The built-in coordinator is now the public one:

`https://quotavpn.duckdns.org/spaces`

This build no longer falls back to the dead `https://quotavpn.duckdns.org:8445`
address. A stored value that exactly matches that dead address is migrated on
startup. A custom coordinator, including a LAN test address, is kept as the
user set it; the default only applies when nothing was stored.

The LAN address seen on the test PC was never a code default. It was saved
into `config.json` while testing the local coordinator and survived the
upgrades. This build removes that ambiguity from a fresh install.

## Spaces console

The page remains the single network console from exp.1:

- A slim rail lists the spaces, the coordinator, and the join/create actions.
- The stage centres the PC in an orbit of members, with paths that draw in and
  a live status band underneath.
- A quiet inspector holds the invite code, the roster, the direct path probe,
  and leave/delete.
- The status word crossfades between Waiting, Ready, Connecting, and Connected.
  The selected space carries a sliding accent, and the centre ring follows the
  connection state.

## Direct path

The owner connects out and the joiner listens. Verified end to end against a
VPS peer: Noise handshake, `QuotaSpace8007` up, and a 4 of 4 ping at 96 to
137 ms. The same transport is in this build.

## Friend test

Both PCs install this build. For a remote friend, create the space on the
public coordinator so the invite carries `https://quotavpn.duckdns.org/spaces`.
Do not reuse a space that was created on a LAN coordinator.

1. Install on this PC.
2. Send the same installer to the friend.
3. You are the owner and connect out; the friend joins and listens.
4. Watch the top status band, not the old Direct path test.

If the friend's router refuses the outbound connection, the relay is the next
fallback.

## Notes

- `0.7.6-exp.1` is deleted. This is the only experimental release.
- A 0.7.5 install on the experimental channel sees `0.7.6-exp.2` as newer.
- The installer is unsigned and will trigger SmartScreen.
