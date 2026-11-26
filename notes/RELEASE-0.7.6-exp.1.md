# QuotaVPN 0.7.6-exp.1

One experimental build for the 0.7.6 line. This is the only experimental
release kept. Stable remains 0.7.5, and the phone remains 0.7.5.

## Spaces, redesigned

The page is now one network console instead of a stack of cards:

- A slim rail lists the spaces, the coordinator, and the join/create actions.
- The stage centres the PC in an orbit of members, with connection paths that
  draw in, member nodes that spring in, and a live status band underneath.
- A quiet inspector holds the invite code, the roster, the direct path probe,
  and leave/delete.
- The status word crossfades between Waiting, Ready, Connecting, and Connected.
  The selected space carries a sliding accent, and the centre ring pulses with
  the connection state.
- The page fills the window and keeps the same glass and night-scene tokens as
  the rest of the app.

## Version line

- `0.7.6-exp.1` is a pre-release of `0.7.6`, not a number on top of the stable
  line.
- A future stable `0.7.6` sorts above it, so making the stable release will not
  be older than the experimental by number.
- The desktop version parser now understands semver pre-releases, with tests:
  `0.7.6 > 0.7.6-exp.1`, `0.7.6-exp.1 > 0.7.5`, and `0.7.6-exp.2 >
  0.7.6-exp.1`.
- The old experimental releases `0.7.6` through `0.7.16` were deleted. This is
  the only experimental build left.

## Direct path

The owner connects out and the joiner listens. This was verified end to end
against a VPS peer on 0.7.16: the owner connected out, the Noise handshake
completed, `QuotaSpace8007` came up, and `ping 10.77.2.3` returned 4 of 4 at
97 to 137 ms. The same code is in this build.

## Friend test

Both PCs install this build. The space and invite do not change.

1. Install on this PC.
2. Send the same installer to your friend.
3. You are the owner and connect out; your friend joins and listens.
4. Watch the top band, not the old Direct path test.

If the friend's router refuses the outbound connection, the relay is the next
fallback.

## Notes

- The old `0.7.16` install will not auto-download this build because the number
  is lower. Install this file manually; the config and spaces are kept.
- A 0.7.5 install on the experimental channel will see `0.7.6-exp.1` as newer.
- The installer is unsigned and will trigger SmartScreen.
