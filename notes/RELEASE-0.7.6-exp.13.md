# QuotaVPN 0.7.6-exp.13

Windows experimental build. The phone stays at 0.7.5.

## Interface scale

- Appearance has a new Interface scale row: 90, 100, 110, 125 and 150 percent.
- It scales the whole webview, including the fixed sizes, and it is saved under
  `qc-scale`.
- 100 percent is the default. The layout was checked at 125 percent.

## Spaces

- Disconnect now leaves the space cleanly. The server promotes another member
  and keeps the network, so the owner leaving no longer takes everyone down.
- Switching to another space while connected uses the switch path: it leaves
  the current space first, then joins the selected one. The old red
  "did not attach" message is gone.
- Creating or joining a space while not connected no longer connects on its
  own. The new space waits for Connect.
- When the selected space disappears, the page stays unselected instead of
  jumping to another space.
- The owner Remove dialog now offers Leave space as the primary action and
  Delete space as the explicit destructive one.
- The switch wait tolerates the engine's delayed network list, and a stale
  error clears once the selected space is actually connected.

## Everything from exp.12

Themes, fonts, the real icon and the readable primary labels remain unchanged.
